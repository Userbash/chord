# POOL-03 — новый пул задач, приоритеты и распределение

Дата: 2026-10-24. PM. Основание: `FIX-REPORT`, `SEARCH-FIX`, `QA-GATE-SKIPS`,
`REVIEW-L02-UI`, `G6-REPARITY-03`, `QA-MATRIX-02`, `V01-SESSION-CONTRACT`, `POOL-02`.

## 1. Состояние на входе

| Артефакт | Итог |
|---|---|
| **FIX-L02-CORE** | готов к review: 577 core / 23 net; P1 flush + md5 + token oracle |
| **L-02-UI** | **RETURN** от Codex: P2 пустой READY, P2 self-ID unsafe |
| **SEARCH-FIX** | 2 новых теста, 139 targeted pass; false-green закрыт на unit |
| **QA-GATE-SKIPS** | e2e skip теперь fail/unmet; 17 meta pass; release без Chrome — красный |
| **C-02-BEFORE** | backend 12 тестов; UI older ветки нет |
| **G6-REPARITY-03** | `deferred`; 0 live, e2e 0/7 browser |
| **QA-MATRIX-02** | M2 не принят; 7 P1 |
| **G3 G-05** | rev.4, 10 пунктов, offline 2/10 |
| **V-01** | контракт состояния RTC готов; кода нет |
| **SEC-L02** | отчёта нет; retry |

### Главные блокеры

1. L-02-UI P2 из Codex review.
2. L-02-CORE fix ждёт повторного Codex review.
3. e2e: 7 browser-сценариев отсутствуют; без Chrome `gate:release` красный.
4. C-02 older UI не подключён.
5. M-07 badge/flags не спроецированы.
6. G-05 2/10 offline; 8 пунктов blocked RE/capability.
7. live E3=0.

---

## 2. Приоритеты

### P0 — live/security stop-line

| ID | Задача | Owner | Статус |
|---|---|---|---|
| P0-01 | live READY | PM+G1+G4 | blocked окном |
| P0-02 | live send | PM+G1+G4 | blocked P0-01 |
| P0-03 | live no-secrets | G4b | offline review active; live blocked |

### P1A — доверие/приёмка

| ID | Задача | Owner | Зависит |
|---|---|---|---|
| **L02-UI-FIX** | пустой READY + self-ID precision | G2 | Codex review |
| **REVIEW-L02-CORE-FIX** | повторный review P1 фиксов | Codex | FIX-REPORT |
| **QA-M2-02** | browser harness + 7 сценариев | G4 | QA plan |
| **QA-M2-04** | pinned Chromium CI job | G5 | CI |
| **SEC-L02-FINAL** | secrets/backpressure | G4b | CORE/UI |
| **G6-REPARITY-04** | финальная матрица | G6 | reports |
| QA-M2-05/07 | test-impact + harness fidelity | G4 | QA-M2-02 |
| L-02-UI-REVIEW-02 | повторный review UI | Codex | L02-UI-FIX |

### P1B — паритет

| ID | Задача | Owner | Зависит |
|---|---|---|---|
| **M-07-BE** | badge/flags projection | G1 | READY/read_state |
| C-02-UI-OLDER | `position:latest` + scroll-up `before` | G2 | backend готов |
| P-03-VERIFY | typing UI offline + TYPING_START gap | G2 | backend/ядро |
| G-01b | `lastGuildId` restore | G2+G1 | UiSettings |
| M-07-UI | unread/badge на экране | G2 | M-07-BE |
| G-05-REST | 8/10 меню | G2+G3 | capability/RE-2 |
| M-06-LIVE | reactions PUT/DELETE/429 | G2+G4 | live |
| G-03-LIVE | invite TTL/USER_BANNED | G1+G2 | live RE |

### P2/P3

| ID | Задача | Owner | Примечание |
|---|---|---|---|
| G3-ST-01 | 63 DOM/17 подпунктов спека | G3 | active |
| V-01-IMPL | RTC session manager offline | G1/G5 | contract ready |
| V-04/05 | Opus/ALSA | G5 | KAT готов |
| ST-01, N-01…03 | settings/notifications | G2/G5 | после P1 |
| M-10/11 | pins/threads | G2+G1 | P2 |
| AT / S-01…03 | attachments/video | G1+G2/G5 | blocked |
| QR/passkey, tray/updater | P3 | G1/G5 | backlog |

---

## 3. Текущая нагрузка

| Отдел | Задача | Режим | Файловые границы |
|---|---|---|---|
| **G2** | **L02-UI-FIX** | writer | `ui/features/dispatch-*`, `ui/app.js`, `dispatch_ui.test.mjs` |
| **G1** | **M-07-BE** | writer | `data.rs`, read_state tests |
| **G4** | **QA-M2-02** | writer | `tests/e2e/**`, `tests/helpers/**`, `tests/fixtures/ui/**` |
| **G5** | **QA-M2-04** | writer | `.github/workflows/**`, `scripts/ci.sh` |
| **G4b** | SEC-L02-FINAL | read-only | security report |
| **G6** | G6-REPARITY-04 | read-only | parity report |
| **G3** | G3-ST-01 | read-only | settings spec |
| **Codex** | REVIEW-L02-CORE-FIX | review-only | no edits |

Один `cargo` — только G1. G2/G4/G5 — node/CI/YAML. Writer-файлы не пересекаются.

---

## 4. Следующие слоты

| Освободится | Следующая | Owner |
|---|---|---|
| G2 после UI FIX | C-02-UI-OLDER → P-03-VERIFY → M-07-UI | G2 |
| G1 после M-07 | PLAT-11a → G-01b contract → V-01 offline | G1 |
| G4 после QA-M2-02 | QA-M2-05/07 → QA-DEDUP impl | G4 |
| G5 после QA-M2-04 | ALSA null → Opus FFI | G5 |
| G4b после SEC | PLAT-11b → Q-08 unsafe | G4b |
| G6 после REPARITY | merge audit → voice matrix | G6 |
| G3 после ST-01 | DP-02/STREAM | G3 |

---

## 5. DoR / DoD

**DoR:** evidence `[L]/[S]/[D]`, файловый владелец, oracle, red-first, зависимости,
live-gate, DoD.

**DoD:** red/green, targeted, tests не ослаблены, review, G6, gate, PM accept; для live — E3.

## 6. Стоп-условия

- e2e skip ≠ pass;
- `passed_with_skips` не приёмка M2;
- P0/P1 не обходятся;
- merge только после gate + parity + accepted set;
- live без окна/токена не запускается.
