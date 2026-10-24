# LOAD-BALANCE — балансировка отделов и Orca-воркеров

Дата: 2026-10-24. PM. Решение владельца: поднять параллелизм, не ломая
TDD/файловые границы/один `cargo` на target-dir.

## 1. Текущая нагрузка

| Лейн | Кто | Задача | Статус |
|---|---|---|---|
| W1 writer | G1 | L-02-CORE (`term_c6f54207…`) | active, current checkout |
| R0 audit | G4 | QA-DEDUP | done (`QUALITY/QA-DEDUP-AUDIT.md`) |
| G2 | — | C-02-PAGE | accepted forward-slice; C-02-BEFORE queued |
| G3 | — | G-05 design | idle |
| G4 | — | QA-E2E | idle/blocked browser |
| G4b | — | security | idle |
| G5 | — | voice prereq | idle |
| G6 | — | parity/merge | idle |

Факт: **1 writer + 0 read-only**; G2/G3/G4/G5/G6 простаивают. 36 partial и 34 absent
не двигаются, потому что critical path L-02 и merge заморожены.

## 2. Ограничители

| Ограничитель | Влияние | Обход |
|---|---|---|
| Dirty tree, HEAD `0239543` | worktree от HEAD не видит RW-39/RW-40 | один writer в текущем checkout; worktree — после commit координатора |
| Один writer на checkout | нельзя параллелить G1/G2/G3-код | отдельные worktree после baseline |
| Один `cargo` на target-dir | Rust-тесты сериализуются | rust-тесты только в integration lane; read-only не запускают cargo |
| Gate/T2 дорогой | нельзя гонять на каждую карточку | T0 targeted; T1 batch; T2 release |
| e2e skip | нельзя объявить pass | отдельный QA-E2E lane, `CHROME_BIN`/`CDP_ENDPOINT` |
| PM не коммитит | baseline не создаётся | координатор L1 после batch |

## 3. Целевой WIP (owner override)

| Волна | Writers | Read-only | Всего | Условие |
|---|---:|---:|---:|---|
| **Сейчас** | 1 (current checkout) | 5 (G6/G4/G2/G5/G3) | **6** | OpenCode DeepSeek; read-only не пишут product, не cargo |
| После baseline commit | 2–3 (worktrees) | 2–3 | **5–6** | файловые множества не пересекаются, cargo один |

Правило: writer-задачи с общими файлами (`chat.js`, `data.rs`, `main.rs`) **не** идут
одновременно в одном checkout. Read-only отчёты — только свои файлы в
`waves/RW-40/{PARITY,QUALITY,DESIGN}/`.

## 4. Перераспределение задач

| Задача | Было | Стало | Причина |
|---|---|---|---|
| L-02-CORE | G1+G2 | **G1** | core hook; UI отдельно |
| L-02-UI | G1+G2 | **G2** | projections/UI; ждёт core |
| C-02-BEFORE | G2 | **G1** | Rust `data.rs`, before-cursor |
| C-02-PAGE | G2 | accepted | forward-slice; не переоткрывать |
| P-03 UI | G2 | **G2** | backend готов; UI+троттлинг |
| G-05 8/10 | G2+G3 | **G3 design → G2 code** | дизайн отдельно, код G2 |
| G-01b | G2+G1 | **G2 + G1 contract** | `UiSettings` поле → UI restore |
| M-07 unread | G1+G2 | **G1 projection → G2 UI** | badge/flags backend |
| QA-E2E | G4 | **G4 + G6 fixtures** | сценарии/CHROME_BIN |
| QA-DEDUP | G4 | **G4** | аудит done, реализация после L-02 |
| PLAT-11a | G1 | **G1**, review G4b | channelId allowlist |
| V-04/V-05 | G5+G1 | **G5 owner**, G1 session | Opus/devices |
| V-06 | G2 | **G2**, после V-05 | UI без фейкового voice |
| G6 parity | G6 | **G6**, PM accept | не PM-код |

## 5. Параллельные пакеты

### Пакет A — critical path (не параллелится с собой)
```
L-02-CORE (W1)
  → L-02-UI (W1 или worktree)
  → G6 re-parity
  → merge
```
Не запускать второй L-02 worker: это тот же контракт.

### Пакет B — read-only (можно сейчас)
- **G6-MERGE-AUDIT**: accepted set vs manifest, stale/missing, merge order.
- **QA-E2E-PREP**: 7 сценариев, 4 skip, CHROME_BIN/CDP, pass criteria.
- **G2-L02-UI-CONTRACT**: event→UI projections, state/dedup/order, test matrix — active.
- **G5-VOICE-PREREQ**: portal/consent, libopus KAT, ALSA null, PipeWire — active.

### Пакет C — G2 writer после L-02-CORE
- L-02-UI → P-03 UI → G-05 8/10 → G-01b → M-07 unread UI.
Один G2 writer; `chat.js`/`typing-indicator.js` общие, не параллелить.

### Пакет D — G4/G6 после baseline
- QA-E2E implement (fixtures/browser), QA-DEDUP implementation, parity tests.
- G6 re-parity; не смешивать с code merge.

## 6. Очередь на свободные слоты

1. **G1**: C-02-BEFORE (после L-02-CORE) → PLAT-11a → SEC-02/PLAT-01.
2. **G2**: L-02-UI (после core) → P-03 UI → G-05 8/10 → G-01b.
3. **G3**: G-05 8-item spec → DP-02/ST-01 review → VOICE/STREAM.
4. **G4**: QA-E2E prep/implement → QA-DEDUP implementation → parity tests.
5. **G4b**: L-02 secrets/log audit → PLAT-11 → Q-08 unsafe registry.
6. **G5**: VOICE-PREREQ → portal/libopus KAT → ALSA null → devices.
7. **G6**: merge audit → re-parity after L-02 → voice/video matrix.

## 7. Что не делать

- не запускать второй writer в текущем checkout;
- не делать worktree от dirty HEAD без baseline;
- не запускать два `cargo` одновременно;
- не выдавать read-only отчёт за implementation;
- не считать e2e skip pass;
- не мержить до G6 parity + gate + accepted set;
- не запускать live без окна/токена.

## 8. Проверка баланса

- writer utilization: 1/1 current; target 2–3/3 after baseline;
- read-only: 3/4 now; target 4/4;
- P1A blockers: L-02-CORE, QA-E2E, G6 parity — все в работе/подготовке;
- G2/G3/G4b/G5 не idle: получают read-only/design lane;
- cargo: 0 параллельных rust-тестов;
- merge: заморожен до L-02 + G6.
