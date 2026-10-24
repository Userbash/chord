# POOL-05 — следующая очередь после текущей волны

Дата: 2026-10-24. PM. Текущая волна: `POOL-04`. Это очередь на освобождающиеся слоты.

## 1. Текущая волна (не дублировать)

| Задача | Owner | Режим | Handle |
|---|---|---|---|
| C-02-UI-OLDER | G2 | writer | `term_58e9f3a2…` |
| G-01b-CONTRACT | G1 | writer | `term_78500e18…` |
| QA-M2-02-VERIFY | G4 | writer | `term_c8cf13b1…` |
| QA-M2-04-VERIFY | G5 | writer | `term_592b8fc9…` |
| SEC-L02-FINAL | G4b | read-only | `term_e9528cd2…` |
| MERGE-READINESS-05 | G6 | read-only | `term_93810767…` |
| G3-STREAM-REVIEW | G3 | read-only | `term_50483361…` |
| REVIEW-L02-UI-FIX | Codex | review-only | `term_5da6aedb…` |

Все слоты заняты; новые задачи выдаются при завершении, без простоя.

## 2. Очередь P0

| ID | Задача | Owner | Условие |
|---|---|---|---|
| P0-01 | live READY | PM+G1+G4 | окно + токен |
| P0-02 | live send | PM+G1+G4 | P0-01 |
| P0-03 | live no-secrets | G4b | live-окно |

## 3. Очередь P1A

| ID | Задача | Owner | Зависит |
|---|---|---|---|
| L02-UI-ACCEPT | принять L-02 UI после Codex | PM | REVIEW-FIX |
| L02-CORE-ACCEPT | PM accept | PM | REVIEW-FIX ACCEPT |
| GATE-GREEN | 16 tsc + 2 plan-sync, `gate:worker` | G4/TL | остановленное дерево |
| QA-M2-05 | test-impact профили | G4 | QA-M2-02 |
| QA-M2-07 | meta harness ↔ real index | G4 | QA-M2-02 |
| SEC-FIX | исправить P1 из SEC-L02 | G1/G4b | SEC-L02 |
| MERGE-06 | accepted set + gate + coordinator | G6/PM | GATE-GREEN |
| QA-DEDUP-IMPL | дубли help/version/Frida | G4 | P1A |

## 4. Очередь P1B

| ID | Задача | Owner | Зависит |
|---|---|---|---|
| M-07-UI | badge/flags на экране | G2 | M-07-BE готов |
| G-01b-UI | restore lastGuildId | G2 | G-01b contract |
| P-03-VERIFY | typing UI + TYPING_START gap | G2/G1 | backend |
| C-07-FIX | QuickSwitcher `data-guild-id` | G2 | rail |
| G-05-REST | 8/10 меню | G2+G3 | capability/RE-2 |
| M-06-LIVE | reactions live | G2+G4 | live |
| G-03-LIVE | invite TTL/USER_BANNED | G1+G2 | live RE |

## 5. Очередь P2/P3

| ID | Задача | Owner | Статус |
|---|---|---|---|
| V-01-IMPL | RTC session manager offline | G1/G5 | contract ready |
| V-04/05 | Opus/ALSA | G5 | KAT ready |
| ST-01-DELTAS | 63 DOM/5 групп/6 voice | G2+G3 | spec ready |
| DP-02 | viewport/zoom review | G3 | P2 |
| STREAM-IMPL | Go Live | G5+G2 | blocked RE-3 |
| N-01…03 | notifications/DND | G5+G2 | P2 |
| M-10/11 | pins/threads | G2+G1 | P2 |
| AT-01…03 | attachments | G1+G2 | blocked upload |
| S-01…03 | camera/Go Live | G5+G1 | blocked live/RE |

## 6. Распределение по отделам

| Отдел | Текущее | Следующее |
|---|---|---|
| **G1** | G-01b contract | PLAT-11a → V-01 offline → SEC-FIX |
| **G2** | C-02 UI older | M-07 UI → P-03 verify → G-01b UI → G-05 |
| **G3** | STREAM review | ST-01 deltas → DP-02 → voice panel |
| **G4** | QA-M2-02 | QA-M2-05/07 → QA-DEDUP → gate green |
| **G4b** | SEC-L02 | PLAT-11b → Q-08 unsafe |
| **G5** | QA-M2-04 | ALSA null → Opus FFI → V-05 |
| **G6** | merge readiness | MERGE-06 → voice/video matrix |
| **Codex** | UI review | CORE/UI re-review only |

## 7. Правила выдачи

1. Сначала P1A, затем P1B, P2 только в свободный read-only слот.
2. Не дублировать активную задачу; один task = один worker.
3. Writer-файлы не пересекаются; один cargo.
4. P0 live выше всех, но требует окна.
5. e2e skip ≠ pass; без gate/parity merge не открывать.
6. Завершившийся writer сразу получает следующую P1A/P1B-карточку.
