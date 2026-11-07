# Реестр воркеров и конвейера отделов (RW-40 — 2026-10-30, Wave 8)

## 1. Принятые карточки (26 кодовых срезов + 2 дизайн-спеки + RE/PLAN, снимок 2026-10-30 18:26)

| Карточка | Отдел | Статус | Доказательство / ACCEPT |
|---|---|---|---|
| `RE-DEEP-REA` | RE | **DONE** | `waves/RW-40/RE-DEEP-REA.md`, `waves/RW-40/ATTACH-RE.md` |
| `PLAN-SYNC` | PM/TL | **DONE** | `TDD-PLAN.md` §8 (`102/102` meta pass, `tsc` exit 0, `0` `@ts-ignore`) |
| `G-01b-CONTRACT` | G1 | **ACCEPTED** | `PM/g01b-contract/REVIEW.md` → `PM/g01b-contract/ACCEPT.md` |
| `QA-M2-04` | G4 | **ACCEPTED** | `QUALITY/QA-M2-04-REVIEW.md` → `QUALITY/QA-M2-04-ACCEPT.md` |
| `L-02` (интеграция) | G1/G2 | **ACCEPTED** | `waves/RW-40/PM/l02/ACCEPT.md` |
| `L-02-CORE` | G1 | **ACCEPTED** | `waves/RW-40/PM/l02-core/REVIEW-FIX.md` → `waves/RW-40/PM/l02-core/ACCEPT.md` |
| `L-02-UI` + `TSC-FIX` | G2/G4 | **ACCEPTED** | `waves/RW-40/PM/l02-ui/REVIEW-PASS2.md` + `MUTATION.md` (5/5 killed) → `waves/RW-40/PM/l02-ui/ACCEPT.md` |
| `C-02-BE` | G1 | **ACCEPTED** | `waves/RW-40/PM/c02-be/ACCEPT.md` |
| `C-02-UI-OLDER-FIX` | G2 | **ACCEPTED** | `waves/RW-40/PM/c02-ui-older/REVIEW-PASS2.md` → `waves/RW-40/PM/c02-ui-older/ACCEPT.md` |
| `PLAT-11a` | G1/G5 | **ACCEPTED** | `waves/RW-40/PM/plat-11a/REVIEW.md` (15/15 contract, 5/5 mutants) → `waves/RW-40/PM/plat-11a/ACCEPT.md` |
| `M-07-BE3` | G1 | **ACCEPTED** | `waves/RW-40/PM/m07-be3/REVIEW-PASS2.md` → `waves/RW-40/PM/m07-be3/ACCEPT.md` |
| `SEC-L02-01-FIX` | G1 | **ACCEPTED** | `waves/RW-40/PM/sec-l02-01/REVIEW.md` (**PASS**, P1 закрыт) → `waves/RW-40/PM/sec-l02-01/ACCEPT.md` |
| `B7b-SEARCH-FILTERS-BACKEND` | G1 | **ACCEPTED** | `waves/RW-40/PM/b7b-search/REVIEW.md` (**PASS**, 16/16 pass, 6/6 mutants) → `waves/RW-40/PM/b7b-search/ACCEPT.md` |
| `M-07-UI-BADGE` | G2 | **ACCEPTED** | `waves/RW-40/PM/m07-ui-badge/REVIEW-PASS2.md` (**PASS**, 29/29 pass) → `waves/RW-40/PM/m07-ui-badge/ACCEPT.md` |
| `G3-STREAM-FIX-PASS2` | G3 | **ACCEPTED** | `waves/RW-40/DESIGN/STREAM-REVIEW-PASS3.md` (**PASS**, F-02..F-04 закрыты) |
| `G3-TYPING-AND-MARKDOWN-SPEC` | G3 Design | **ACCEPTED** | `waves/RW-40/DESIGN/TYPING-MARKDOWN-SPEC-REVIEW-PASS4.md` (**PASS**, токены `tokens.css`) |
| `STUB-27-CORE-ORACLE-FIX` | G1 Core | **ACCEPTED** | `waves/RW-40/PM/stub27-core/ACCEPT.md` (11/11 pass, 5/5 mutants) |
| `V-01-RTC-FIX` | G5 Voice | **ACCEPTED** | `waves/RW-40/PM/v01-rtc/ACCEPT.md` (147/147 pass, 7/7 mutants) |
| `STUB-05-INVITES-BACKEND` | G1 Backend | **ACCEPTED** | `waves/RW-40/PM/stub05-invites-be/ACCEPT.md` (20/20 pass, 5/5 mutants) |
| `STUB-14-UI-SEARCH-FIX` | G2 Lane B | **ACCEPTED** | `waves/RW-40/PM/stub14-search-ui/ACCEPT.md` (108/108 pass, 5/5 mutants) |
| `P-03-UI-TYPING-INDICATOR` | G2 Lane A | **ACCEPTED** | `waves/RW-40/PM/p03-typing-ui/ACCEPT.md` (38/38 pass, 5/5 mutants) |
| `M-10-M-11-PINS-THREADS-BACKEND` | G1 Backend | **ACCEPTED** | `waves/RW-40/PM/m10-pins-be/ACCEPT.md` (6/6 pass, 5/5 mutants) |
| `STUB-27-CORE-DISPATCH-INTEGRATION` | G1 Core | **ACCEPTED** | `waves/RW-40/PM/stub27-dispatch/ACCEPT.md` (583/583 pass, 5/5 mutants) |
| `STUB-05-INVITES-MODAL-UI` | G2 Lane B | **ACCEPTED** | `waves/RW-40/PM/stub05-invites-ui/ACCEPT.md` (35/35 pass, 5/5 mutants) |
| `M-08-MARKDOWN-MENTIONS-UI` | G2 Lane C | **ACCEPTED** | `waves/RW-40/PM/m08-markdown/ACCEPT.md` (48/48 pass, 5/5 mutants) |
| `V-07-VOICE-DEVICES-CONTRACT` | G5 Voice | **ACCEPTED** | `waves/RW-40/PM/v07-devices/ACCEPT.md` (5/5 pass, 5/5 mutants) |
| `INT-02-UI-CHAT-MARKDOWN-TYPING` | G2 Lane A | **ACCEPTED** | `waves/RW-40/PM/int02-chat-md/ACCEPT.md` (`53/53` + `272/272` регрессий, 5/5 мутаций, P0=P1=P2=0) |
| `B2a-ATTACH-TIER-FIX-PASS2` | G1 Backend | **ACCEPTED** | `waves/RW-40/PM/b2a-attach/ACCEPT.md` (`17/17`, 4/4 мутаций, пол 20 МиБ) |
| `STUB-16-MEMBER-PROFILE-POPOVER-FIX` | G2 Lane C | **ACCEPTED** | `waves/RW-40/PM/stub16-profile-ui/ACCEPT.md` (`13/13`, 5/5 мутаций, P0=P1=P2=0) |
| `STUB-10-PINS-THREADS-PANELS-UI` | G2 Lane C | **ACCEPTED** | `waves/RW-40/PM/stub10-pins-threads/ACCEPT.md` (19/19 pass, 5/5 мутантов killed) |
| `B2b-ATTACH-UI-20MIB-SYNC` | G2 Lane A | **ACCEPTED (PASS-ревью)** | `waves/RW-40/PM/b2b-attach-ui/REVIEW-PASS4.md` (`29/29`, 5/5 caught; отдельного `ACCEPT.md` нет) |

## 2. Завершённые карточки и вспомогательные артефакты

| Карточка | Отдел | Движок / Модель | Выходной артефакт | Статус |
|---|---|---|---|---|
| `B2a-ATTACH-TIER-FIX` | G1 Backend | `antigravity` / `gemini-4-argon-medium` | `waves/RW-40/PM/b2a-attach/REPORT-FIX.md` + `REPORT-PASS3.md` (17/17 pass, 4/4 tier mutants, pass 2 ACCEPT) | **DONE → ACCEPTED** |
| `G-01b-UI-SETTINGS-FIX` | G2 Lane A | `antigravity` / `gemini-3.8-flash-medium` | `waves/RW-40/PM/g01b-ui/REPORT-FIX.md` (112/112 targeted, 776/776 unit) | **DONE** |
| `STUB-16-MEMBER-PROFILE-POPOVER-UI` | G2 Lane C | `antigravity` / `gemini-3.8-flash-high` | `waves/RW-40/PM/stub16-profile-ui/REPORT.md` (11/11 pass) | **DONE** |
| `B2b-ATTACH-UI-FIX` | G2 Lane A | `antigravity` / `gemini-3.8-flash-high` | `waves/RW-40/PM/b2b-attach-ui/REPORT-PASS4.md` + `REVIEW-PASS4.md` (`0` `@ts-ignore`, PUT шаг, 29/29 pass, 5/5 caught) | **DONE → ACCEPTED** (PASS-ревью; P1 по реальному IPC остаётся) |
| `STUB-10-PINS-THREADS-PANELS-UI` | G2 Lane C | `antigravity` / `gemini-3.8-flash-medium` | `waves/RW-40/PM/stub10-pins-threads/ACCEPT.md` (19/19 pass, 5/5 мутантов убиты) | **ACCEPTED** |
| `G3-TYPING-AND-MARKDOWN-SPEC` | G3 Design | `antigravity` / `gemini-3.8-flash-low` | `waves/RW-40/DESIGN/TYPING-MARKDOWN-SPEC-REVIEW-PASS4.md` | **ACCEPTED** (PASS 4) |
| `G6-PARITY-SYNC-09` | G6 Parity | `opencode` / `deepseek/deepseek-flash` | `waves/RW-40/PARITY/MERGE-READINESS-09.md` | **DONE** |
| `INT-01-RUST-WIRING` | G1 Rust Integration | `antigravity` / `gemini-4-argon-high` | `waves/RW-40/PM/int01-rust-wiring/REPORT.md` (5/5 pass, clippy exit 0, 5/5 мутаций, 6 IPC-обёрток) | **REPORT DONE** (ревью 15A нет) |
| `INT-03-SAVE-QUEUE-FIX` | G2 Lane B | `antigravity` / `gemini-3.8-flash-high` | `waves/RW-40/PM/g01b-ui/REPORT-PASS4.md` (98/98 targeted, 5/5 мутаций, `saveQueue`) | **PASS 4 DONE** (ожидает формальный ACCEPT/ревью) |
| `G05-REST-MUTATIONS-FIX` | G2 Lane C | `opencode` / `deepseek/deepseek-flash` | `waves/RW-40/PM/g05-rest/REPORT-PASS3.md`, `REVIEW-PASS4.md` (833/833 unit, 5/5 мутаций) | **REVIEW RETURN** (P0=0, P1=1: нет `gate:worker`) |
| `G6-PARITY-SYNC-11` | G6 Parity | `opencode` / `deepseek/deepseek-flash` | `waves/RW-40/PARITY/MERGE-READINESS-11.md` (24 среза, 833/833, `23/41/27`) | **DONE** |
| `G6-PARITY-SYNC-12` | G6 Parity | `antigravity` / `gemini-3.8-flash-low` | `waves/RW-40/PARITY/MERGE-READINESS-12.md` (26 срезов, 834/834, `saveQueue`) | **DONE** |

## 3. Конвейер Wave 8 на снимке 2026-10-30 18:26

| Отдел / Лейн | Терминал (`handle`) | Движок (`--agent`) | Модель и `--effort` | Карточка, файлы и статус |
|---|---|---|---|---|
| **G1 Rust Integration** | `term_dc83964e-a040-450a-901c-654aed99d200` | `antigravity` | `gemini-4-argon-high` (`high`) | `INT-01-RUST-WIRING`: 6 IPC-обёрток в `lib.rs`/`main.rs`, 5/5 pass, clippy exit 0 — **отчёт сдан, ждёт Codex 15A** |
| **G2 UI Integration Lane A** | `term_a34ae4d3-8a63-493d-b1b4-357c3e2c371d` | `antigravity` | `gemini-3.8-flash-high` (`high`) | `INT-02-UI-CHAT-MARKDOWN-TYPING`: `chat.js` + `composer.css` — **ACCEPTED** (`PM/int02-chat-md/ACCEPT.md`) |
| **G2 UI Integration Lane B** | `term_a2791178-6f2b-48e5-a87a-e6d6971e7af9` | `antigravity` | `gemini-3.8-flash-high` (`high`) | `INT-03-SAVE-QUEUE-FIX`: `app.js` (`saveQueue`) + `guild-rail.test.mjs` — **PASS 4 DONE** (98/98 targeted, 5/5 killed) |
| **G2 Fix Lane C** | `term_bd8922bd-4e58-4a05-855f-618c78bf1ba2` | `opencode` | `deepseek/deepseek-flash` | `G05-REST-MUTATIONS-FIX`: guard в `actions.js` + `server-menu.test.mjs` — **RETURN, P1** (нет `gate:worker`) |
| **G6 Parity** | `term_g6_parity_current` | `antigravity` | `gemini-3.8-flash-low` | `G6-PARITY-SYNC-12`: `waves/RW-40/ORCA-WORKERS.md`, `waves/RW-40/PARITY/MERGE-READINESS-12.md`, `waves/RW-40/PARITY/PARITY-MATRIX.md` — **DONE** |
| **Codex Review 15A** | `term_4cf2beb5-bd8b-4482-a4f0-625cce0f878a` | `codex` | `gpt-6-luna` (`high`) | `REVIEW-BATCH-15A`: ревью `INT-01-RUST-WIRING` и `INT-02-UI-CHAT-MARKDOWN-TYPING` — **ожидает** |
| **Codex Review 15B** | `term_3d33184b-37c5-4183-9eab-293f22983164` | `codex` | `gpt-6-luna` (`high`) | `REVIEW-BATCH-15B`: ревью `INT-03` (Pass 4) и `G05-REST-MUTATIONS-FIX` |

### Блокеры слияния на снимке

1. `G05`: P1 — нет evidence `npm run gate:worker` с exit code (`PM/g05-rest/REVIEW-PASS4.md`).
2. IPC-имена: UI вызывает `attachments_upload_slots`, `attachments_upload_put`, `create_invite`, `pins_list`, `threads_list`; в `main.rs` их нет (находка G6 `PARITY-SYNC-11`, P1).
3. Gate-артефакт последний раз записан `2026-10-24T10-22-26-400Z` на коммите `0239543`; на HEAD `4e24048` gate и интеграционного коммита нет.
