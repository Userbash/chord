# POOL-06 — текущая волна, приоритеты и распределение

Дата: 2026-10-24. PM. После integration commit `4e24048`.

## 1. Снимок

| Проверка | Факт |
|---|---|
| Remote | `4e24048` отправлен |
| Дерево | clean на момент коммита |
| L-02-CORE | Codex `ACCEPT`, PM-accept pending |
| L-02-UI | **RETURN**: P2 fixed, нет mutation evidence |
| C-02-UI-OLDER | готово, Codex review active |
| M-07-BE3 | готово, UI не читает badge |
| G6 | merge **block**, parity **deferred** |
| Gate | red: 16 `tsc` в `dispatch_ui.test.mjs` + 10 plan-sync файлов |
| e2e | 0/7 browser; без Chrome release красный |
| live | E3=0 |

## 2. Активная волна

| Отдел | Задача | Режим | Handle |
|---|---|---|---|
| G2 | L02-UI-MUTATION | read-only | `term_b8090ae7…` |
| G4 | TSC-FIX | writer | `term_c9b00c45…` |
| G1 | G-01b-CONTRACT | writer | `term_e178238f…` |
| G4b | SEC-L02-FINAL | read-only | `term_34f97fb1…` |
| G5 | QA-M2-04-VERIFY | writer | `term_f8f5b226…` |
| G6 | MERGE-READINESS-06 | read-only | `term_056ae0e8…` |
| G3 | G3-STREAM-FIX | writer (docs) | `term_5ea23572…` |
| Codex | REVIEW-C02-UI | review-only | `term_764e5e0d…` |

## 3. Приоритеты

### P0 — live

| ID | Задача | Статус |
|---|---|---|
| P0-01 | live READY | blocked окном |
| P0-02 | live send | blocked P0-01 |
| P0-03 | live no-secrets | offline SEC; live blocked |

### P1A — приёмка/merge

| ID | Задача | Owner | Статус |
|---|---|---|---|
| L02-UI-MUTATION | mutation evidence | G2 | active |
| REVIEW-C02-UI | Codex review | Codex | active |
| TSC-FIX | 16 → 0 | G4 | active |
| SEC-L02 | security evidence | G4b | active |
| QA-M2-04 | CI report | G5 | active |
| MERGE-06 | accepted set/gate | G6 | active |
| PLAN-SYNC | 10 файлов в TDD-PLAN §8 | PM/TL | queued |
| QA-M2-02 | 7 e2e report verify | G4 | queued |
| L02 ACCEPTs | PM accept CORE/UI | PM | после review |
| GATE-GREEN | `gate:worker` на clean stop | G4/TL | после TSC/plan-sync |

### P1B — паритет

| ID | Задача | Owner | Зависит |
|---|---|---|---|
| G-01b-CONTRACT | `lastGuildId` | G1 | active |
| M-07-UI | badge/flags на экране | G2 | BE ready |
| G-01b-UI | restore рейла | G2 | contract |
| C-02-UI-CSS | top-history bar styling | G2 | C-02 UI |
| P-03-VERIFY | typing UI/TYPING_START gap | G2/G1 | backend |
| C-07-FIX | QuickSwitcher `data-guild-id` | G2 | rail |
| G-05-REST | 8/10 меню | G2+G3 | capability/RE |
| M-06-LIVE | reactions live | G2+G4 | live |

### P2/P3

| ID | Задача | Owner | Примечание |
|---|---|---|---|
| V-01-IMPL | RTC session manager offline | G1/G5 | contract ready |
| V-04/05 | Opus/ALSA | G5 | KAT ready |
| G3-STREAM-FIX | F-01…F-04 docs | G3 | active |
| ST-01-DELTAS | 63 DOM/5 групп/6 voice | G2+G3 | spec ready |
| STREAM-IMPL | Go Live | G5+G2 | blocked RE-3 |
| N-01…03 | notifications/DND | G5+G2 | P2 |
| M-10/11 | pins/threads | G2+G1 | P2 |
| AT/video | attachments/Go Live | — | blocked |

## 4. Файловые границы

```
G4  tests/unit/dispatch_ui.test.mjs (types only)
G1  settings_store.rs + settings tests
G5  .github/workflows/** + scripts/ci.sh
G3  DESIGN/STREAM.md (+ отчёт)
G2  только отчёт mutation; product не меняет
G4b/G6  только отчёты
Codex  только review
```

Один `cargo` — G1. G4/G5 — node/YAML. G3 — docs.

## 5. Следующие слоты

| Освободится | Следующая | Owner |
|---|---|---|
| G2 после mutation | M-07-UI → G-01b-UI → P-03 | G2 |
| G1 после G-01b | V-01 offline → PLAT-11a | G1 |
| G4 после TSC | QA-M2-02 verify → QA-M2-05/07 | G4 |
| G5 после CI | ALSA null → Opus FFI | G5 |
| G4b после SEC | PLAT-11b → Q-08 | G4b |
| G6 после merge | voice/video matrix | G6 |
| G3 после STREAM | ST-01 deltas → DP-02 | G3 |

## 6. Ближайшая цель

1. Mutation evidence → L-02-UI accept.
2. TSC 16 → 0; plan-sync 10; `gate:worker` зелёный.
3. SEC-L02 + QA-M2-04 reports.
4. C-02 UI review accept.
5. `MERGE-READINESS-06` → coordinator.
6. Затем M-07 UI / G-01b / live P0 / voice.
