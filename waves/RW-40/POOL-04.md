# POOL-04 — текущий пул, близость к цели и приоритеты

Дата: 2026-10-24. PM. Снимок после `FIX-L02-CORE`, `L02-UI-FIX`, `M-07-BE3`,
`QA-GATE-SKIPS`, `G6-REPARITY-04`.

## 1. Насколько близко к цели

### Offline-паритет

| Метрика | Значение |
|---|---:|
| Всего функций | **91** |
| implemented (offline) | **21** |
| partial | **36** |
| absent | **34** |
| verified live (E3) | **0** |
| parity-ok строк | **0** |

Вывод: **широта offline-кода ~60%** (57/91 имеют код), но **приёмка/паритет существенно
ниже**: 21 implemented = 23%, 0 live. Нельзя считать «почти готово».

### Merge readiness

| Условие | Статус |
|---|---|
| Gate | **red**: `tsc` 16 ошибок в `dispatch_ui.test.mjs`, meta `plan-sync` 2 файла |
| e2e browser | **0/7** сценариев; 2 harness-теста skip без Chrome |
| Accepted set | **2** среза с ACCEPT; ~10 без ACCEPT |
| L-02 UI | **RETURN**, fix готов к повторному review |
| L-02 CORE | Codex **ACCEPT**, PM accept pending |
| G6 parity | **deferred** |
| Live E3 | **0** |

Вердикт G6: **merge = block**, не deferred. До merge нужны: зелёный gate, accepted set,
L-02 UI re-review, без e2e skip.

### Voice / video

- Voice: контракт V-01, KAT, panel spec; **кода session manager нет**, Opus/devices/live = 0.
- Video/Go Live: только STREAM-ревью; **0** реализации, blocked RE/live.
- Attachments: blocked upload-контракт.

Ориентир: **text offline — средняя готовность; merge — далеко; live — 0; voice/video — ранняя стадия.**

---

## 2. Активные задачи сейчас

| Задача | Отдел | Режим | Handle |
|---|---|---|---|
| **C-02-UI-OLDER** | G2 | writer | `term_58e9f3a2…` |
| **G-01b-CONTRACT** | G1 | writer | `term_78500e18…` |
| **QA-M2-02-VERIFY** | G4 | writer | `term_c8cf13b1…` |
| **QA-M2-04-VERIFY** | G5 | writer | `term_592b8fc9…` |
| **SEC-L02-FINAL** | G4b | read-only | `term_e9528cd2…` |
| **MERGE-READINESS-05** | G6 | read-only | `term_93810767…` |
| **G3-STREAM-REVIEW** | G3 | read-only | `term_50483361…` |
| **REVIEW-L02-UI-FIX** | Codex | review-only | `term_5da6aedb…` |

4 writer (G2/G1/G4/G5) с непересекающимися файлами; один cargo (G1);
G4b/G6/G3 — read-only; Codex — только review.

---

## 3. Приоритеты пула

### P0 — live/security

| ID | Задача | Owner | Статус |
|---|---|---|---|
| P0-01 | live READY | PM+G1+G4 | blocked окном |
| P0-02 | live send | PM+G1+G4 | blocked P0-01 |
| P0-03 | live no-secrets | G4b | offline review active; live blocked |

### P1A — приёмка/доверие

| ID | Задача | Owner | Статус |
|---|---|---|---|
| L02-UI-REVIEW-02 | повторный review UI fix | Codex | active |
| L02-CORE-ACCEPT | PM accept после ACCEPT | PM | pending |
| QA-M2-02 | 7 browser-сценариев | G4 | active verify |
| QA-M2-04 | Chromium CI job | G5 | active verify |
| SEC-L02 | secrets/log/backpressure | G4b | active retry |
| MERGE-READINESS-05 | accepted set/gate | G6 | active |
| GATE-GREEN | tsc 16 + plan-sync 2 | G4/TL | queued |
| QA-M2-05/07 | impact/harness fidelity | G4 | queued |

### P1B — паритет

| ID | Задача | Owner | Статус |
|---|---|---|---|
| C-02-UI-OLDER | latest + before scroll | G2 | active |
| M-07-UI | badge/flags на экране | G2 | queued; BE ready |
| G-01b-UI | restore lastGuildId | G2 | contract active |
| P-03-VERIFY | typing UI / TYPING_START | G2 | queued |
| G-05-REST | 8/10 меню | G2+G3 | blocked capability/RE |
| M-06-LIVE | reactions PUT/DELETE/429 | G2+G4 | live |
| G-03-LIVE | invite TTL | G1+G2 | live RE |

### P2/P3

| ID | Задача | Owner | Статус |
|---|---|---|---|
| G3-ST-01 | 63 DOM/17 подпунктов | G3 | spec ready |
| G3-STREAM | Go Live spec | G3 | active |
| V-01-IMPL | RTC session manager | G1/G5 | contract ready |
| V-04/05 | Opus/ALSA | G5 | KAT ready |
| ST-01/N-01…03 | settings/notifications | G2/G5 | after P1 |
| M-10/11 | pins/threads | G2+G1 | P2 |
| AT/video | attachments/Go Live | — | blocked |

---

## 4. Следующие слоты

| Освободится | Следующая | Owner |
|---|---|---|
| G2 после C-02 | M-07-UI → P-03-VERIFY → G-01b-UI | G2 |
| G1 после G-01b | PLAT-11a → V-01 offline | G1 |
| G4 после QA-M2-02 | QA-M2-05/07 → QA-DEDUP impl | G4 |
| G5 после QA-M2-04 | ALSA null → Opus FFI | G5 |
| G4b после SEC | PLAT-11b → Q-08 | G4b |
| G6 после merge | voice/video matrix | G6 |
| G3 после STREAM | DP-02/ST-01 impl spec | G3 |

---

## 5. Ближайшая цель — merge readiness

1. Codex review L-02-UI-FIX → accept.
2. PM accept L-02-CORE + L-02-UI.
3. Убрать 16 tsc и 2 plan-sync; `gate:worker` зелёный.
4. QA-M2-02/04: harness/CI; e2e 7/7 не skip (или явный infra fail).
5. C-02 UI + M-07 UI + G-01b contract/UI.
6. `MERGE-READINESS-05` → coordinator baseline.
7. Только потом live P0 и voice MVP.
