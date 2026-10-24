# POOL-02 — новый пул задач, приоритеты и распределение

Дата: 2026-10-24. PM. Основание: `G6-REPARITY-02`, `QA-MATRIX-02`, Codex REVIEW,
отчёты L-02-UI/C-02-BEFORE, `LOAD-BALANCE`, `MERGE-POLICY`.

## 1. Вход в пул

### Готово к приёмке/ревью

| Артефакт | Статус |
|---|---|
| L-02-CORE | **RETURN** от Codex: P1 flush-on-error, P1 md5, P2 token oracle |
| L-02-UI | ready к приёмке; Codex review запущен |
| C-02-BEFORE backend | ready; `position:latest` + `before`/`olderCursor` |
| C-02-UI older | не сделано; backend готов |
| G6 re-parity | deferred; blockers перечислены |
| QA matrix | M2 **не принят**; 7 P1 |
| G3 G-05 spec | draft/9 дефектов; исправляется |
| G5 voice prereq | ready; KAT/panel/V-01 контракт |
| SEC-L02 | нет результата; retry |

### Главные блокеры

1. **L-02-CORE P1** — потеря уже разобранных op0 при ошибке в том же WS-фрагменте.
2. **QA-M2-01** — e2e `pass 0 / skip 4` даёт `passed_with_skips` и exit 0.
3. **QA-M2-03** — remote search false-green (`dataset.guild` вместо `data-guild-id`).
4. **L-02 UI не принят** — нужен Codex review и gate.
5. **C-02 older UI** — backend есть, `chat.js` не использует `olderCursor`.
6. **e2e 0/7 browser** — нет harness/CI/CHROME_BIN.
7. **live E3=0** — окно/токен.

---

## 2. Приоритеты и пул

### P0 — live/security stop-line

| ID | Задача | Owner | Статус |
|---|---|---|---|
| P0-01 | live READY | PM+G1+G4 | blocked: окно/токен |
| P0-02 | live send | PM+G1+G4 | blocked: P0-01 |
| P0-03 | live no-secrets | G4b | offline review active; live blocked |

### P1A — доверие/приёмка

| ID | Задача | Owner | Размер | Зависит |
|---|---|---|---|---|
| FIX-L02-CORE | flush-before-error, md5, token oracle | G1 | S | Codex review |
| REVIEW-L02-UI | Codex review UI diff | Codex | S | L-02-UI report |
| SEARCH-FIX | `data-guild-id`, no fallback | G2 | S | QA-M2-03 |
| QA-GATE-SKIPS | skip ≠ pass, failOnSkips | G4 | M | QA-M2-01 |
| SEC-L02 | secrets/backpressure audit | G4b | S | L-02 CORE/UI |
| G6-REPARITY-03 | matrix после L-02 | G6 | M | reports |
| QA-M2-02/04/05/07 | browser harness/CI/impact/meta | G4+G5 | L | CHROME_BIN |

### P1B — паритет

| ID | Задача | Owner | Размер | Зависит |
|---|---|---|---|---|
| C-02-UI-OLDER | `position:latest` + scroll-up `before` | G2 | S | C-02-BEFORE |
| P-03 UI | typing indicator/UI (offline; verify) | G2 | S | backend |
| M-07-UNREAD | badge/flags projection + UI | G1+G2 | M | read_state |
| G-01b | `lastGuildId` restore | G2+G1 | S | UiSettings |
| G-05-REST | 8/10 menu (capability/RE) | G2+G3 | M | spec |
| G-03-LIVE | invite codes/TTL | G1+G2 | M | live RE |
| M-06-LIVE | reaction PUT/DELETE/429 | G2+G4 | S | live |

### P2/P3

| ID | Задача | Owner | Примечание |
|---|---|---|---|
| V-01 | RTC session contract | G5 | **active** |
| V-02/03 | UDP/SRTP live | G1+G4b | blocked live |
| V-04/05 | Opus/ALSA | G5 | KAT ready |
| V-06 | voice panel | G3 | spec active |
| ST-01 | settings 63 DOM/subitems | G2+G3 | P2 |
| N-01…03 | notifications/DND | G5+G2 | P2 |
| M-10/11 | pins/threads | G2+G1 | P2 |
| AT-01…03 | attachments | G1+G2 | blocked upload |
| S-01…03 | video/Go Live | G5+G1 | blocked RE/live |
| A-04/05 | QR/passkey | G1+G5 | P3 |
| PL-01…08 | tray/deeplink/updater | G5 | P3 |

---

## 3. Распределение нагрузки сейчас

| Отдел | Задача | Режим | Файлы/границы |
|---|---|---|---|
| **G1** | FIX-L02-CORE | **writer** | `discord-core/src/ws.rs`, `gateway/supervisor.rs`, core/net tests |
| **G2** | SEARCH-FIX | **writer** | `ui/app.js`, `ui/features/search-overlay.js`, target test |
| **G4** | QA-GATE-SKIPS | **writer** | `scripts/gate.mjs`, meta/test-policy |
| **G4b** | SEC-L02-RETRY | read-only | security report |
| **G6** | G6-REPARITY-03 | read-only | parity report |
| **G3** | G3-G05-DEFECTS | read-only | G-05 spec |
| **G5** | V01-SESSION-CONTRACT | read-only | voice contract |
| **Codex** | REVIEW-L02-UI | review-only | no edits |

Один `cargo` — только G1. G2/G4 — node-тесты. Три writer-а разрешены только потому,
что файловые множества не пересекаются; это временное исключение, не общая норма.

---

## 4. Следующие слоты

| Освободится | Следующая задача | Owner |
|---|---|---|
| G1 после FIX | приёмка CORE + PLAT-11a | G1 |
| G2 после SEARCH | C-02-UI-OLDER → P-03 UI → M-07 UI | G2 |
| G4 после GATE | QA-M2-02 browser harness → test-impact | G4 |
| G4b после SEC | PLAT-11b/Q-08 unsafe | G4b |
| G6 после REPARITY | merge audit update | G6 |
| G3 после G-05 | DP-02/ST-01/stream | G3 |
| G5 после V-01 | ALSA null → libopus FFI | G5 |

Правило: освободившийся writer сразу получает следующую P1A/P1B-карточку; read-only
слот получает следующий аудит/спеку; P2 не вытесняет P1; live — только окно.

---

## 5. Definition of Ready для новой карточки

1. Evidence `[L]/[S]/[D]` или явный blocker.
2. Файловое множество и владелец.
3. Оракул/тест и red-first.
4. Зависимости и live-gate.
5. DoD и что **не** считается pass.

## 6. Стоп-условия

- нет доказательств — не задача, а RE-spike;
- e2e skip ≠ pass;
- P0/P1 не обходим;
- merge только после gate + parity + accepted set;
- live без окна/токена не запускаем.
