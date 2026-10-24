# DEVELOPMENT-PROCESS — разработка, ревью, окно слияния

Дата: 2026-10-24. PM. Сводный процесс. Источники: `PROCESS.md`, `TEST-POLICY.md`,
`MERGE-POLICY.md`, `PIPELINE.md`, `PARALLEL-LANES.md`, `ORCA-WORKERS.md`.

## 1. Карта процесса

```
Owner
  │ цель / live-окно
  ▼
G6 RE Parity ──► decision-ready report ──► PM triage
  (dump/app.asar)      [L]/[S]/[D]             │
                                               ▼
                                        QUEUE / TASKS
                                               │
                     ┌─────────────────────────┼─────────────────────────┐
                     ▼                         ▼                         ▼
                   G1 Backend                G2 Frontend               G3 Design
                   G4 QA                     G4b Security              G5 Platform
                     │                         │                         │
                     └──────────────┬──────────┴──────────┬──────────────┘
                                    ▼                     ▼
                              Orca worker             contract/fake
                              (1 task = 1 worker)      (parallel)
                                    │
                              red → green
                                    │
                              targeted tests
                                    │
                              handoff PM
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
             independent review                batch gate T1
             (not author)                 frontend+rust+gate:worker
                    │                               │
                    └───────────────┬───────────────┘
                                    ▼
                              G6 parity
                                    │
                              PM accept
                                    │
                              integration merge
                              (coordinator commit)
                                    │
                              gate:release T2
                                    │
                              QA handoff / live T3
```

## 2. Роли

| Роль | Кто | Ответственность | Не делает |
|---|---|---|---|
| Owner | человек | цель, приоритет, live-окно | не пишет код |
| PM | DeepSeek | пул, WIP, приоритеты, приёмка, окна | не коммитит, не ревьюит свой код |
| Coordinator / L1 | отдельный | commit/push, integration | не меняет логику |
| G6 | RE Parity | dump/app.asar, evidence, matrix, gaps | не пишет product |
| G1 | Backend/Network | REST/WS, guilds, voice core | не UI |
| G2 | Frontend | UI, DOM/aria, IPC | не backend |
| G3 | Design | SERVER-MENU, VOICE, STREAM, tokens | не код |
| G4 | QA | tests, gate, e2e, mutation | не ослабляет тесты |
| G4b | Security | secrets/logs, unsafe/FFI, consent | не product |
| G5 | Platform | audio/capture/OS, notifications | не UI |
| Orca worker | 1 task | своя карточка и файлы | не трогает чужие файлы |

## 3. Intake: как задача попадает в работу

1. G6 разбирает эталон (`app.asar`/dump/live).
2. G6 пишет: функция, source, `[L]/[S]/[D]`, текущее состояние, gap, контракт, DoD,
   test, owner, P/WSJF, ready/blocked.
3. PM проверяет evidence. Без G6-отчёта задача в пул не попадает.
4. PM добавляет в `QUEUE.md`/`TASKS.md`/`STUB-TRACKER.md`.
5. PM назначает owner и срочность.

Правило: `[L]` live, `[S]` call-site, `[D]` enum — evidence; `[M]/[A]` — не функция.
`POST /typing`, `/invites`, `channels`, `read_state` получили отдельные `[S]`/READY-оракулы.

## 4. Разработка одной задачи

### 4.1 Contract-first

- IPC: имя, args, result, typed errors.
- Fake fixture + negative cases.
- UI может работать на fake, backend — на contract test; merge later.

### 4.2 TDD

```
red (тест падает по правильной причине)
 → минимальный код
 → green targeted
 → handoff
```

- tests: не ослаблять, не менять под код, без `.only/.skip/todo`, без пустых assert.
- oracle: dump/fixture/jq/md5/independent artifact.
- автор не ревьюит свой код.

### 4.3 T0/T1/T2/T3

| Tier | Кто | Что |
|---|---|---|
| T0 | автор | только затронутый test + contract |
| T1 | G4/Integration | full frontend + gui Rust + workspace + `gate:worker` — один раз на batch |
| T2 | PM/G4 | `gate:release`: typecheck/meta/unit/smoke/e2e/coverage/keyring/golden |
| T3 | PM+G4/G1 | live READY/send/voice/keyring locked |

Полный suite не гоняется на каждую карточку. `node scripts/test-impact.mjs` выбирает T0.

## 5. Ревью кода

| Проверка | Кто | Что |
|---|---|---|
| Contract/evidence | G6 | source, `[L]/[S]/[D]`, нет вымысла |
| Tests | G4 | red-first, нет ослаблений, regression |
| Code | независимый reviewer | не автор; P0/P1, границы, ошибки |
| Security | G4b | секреты/логи/unsafe/consent |
| Architecture | Tech Lead | контракты, общие файлы |
| Final | PM | accept только с evidence |

Правила:
- Codex — **только review/security**, не implementation.
- Reviewer не правит код автора; возвращает с repro.
- P0/P1 → return; P2 → deferred с тикетом; P3 → backlog.
- 2 одинаковых отказа → смена подхода; 3-я идентичная попытка запрещена.

## 6. Окно слияния

### 6.1 Когда открывается

Не по календарю, а на **границе batch/vertical slice**:
- 3–6 карточек или законченный срез;
- все targeted green;
- контракт заморожен;
- один integration-исполнитель, один `cargo`.

### 6.2 Проверка готовности

- [ ] только свои файлы;
- [ ] red/green логи;
- [ ] targeted + clippy/fmt/tsc;
- [ ] тесты не ослаблены;
- [ ] md5/handoff;
- [ ] нет P0/P1;
- [ ] G6 evidence;
- [ ] live-функция имеет E3 или `live unverified`.

### 6.3 Процесс

```
handoff
 → independent review
 → batch gate T1
 → G6 parity (parity-ok/deferred/block)
 → PM accept
 → integration merge
 → coordinator commit
 → gate:release T2
 → QA handoff
 → live T3 (отдельное окно)
```

### 6.4 Возврат

Любой P0/P1/gate fail → **та же карточка** `returned`:
```
fix → targeted → batch gate → review → G6 → accept
```
P2 может deferred, P3 backlog. «Принять с красным» нельзя.

### 6.5 Все принятые, не только последний batch

`MERGE-MANIFEST.md`: в integration входят auth, messages, search, settings, QuickSwitcher,
DP-01/02, L-01, SEC, Q-*, V-08 UI, ST-01, PLAT-11 meta, QA tools. D-128 — только новый batch.

## 7. Merge window сейчас

| Условие | Состояние |
|---|---|
| Gate | passed_with_skips |
| G6 parity | **deferred** |
| L-02 dispatch | in work |
| C-02 paging | in work |
| QA-DEDUP | Orca read-only |
| e2e | 4 skip |
| P0 live | blocked |
| Merge | **не выполнять** |

Причины deferred: L-02/G6-D-03, e2e skip, G-05 2/10, M-07 unread, C-02 paging, E3 нет.

## 8. Orca-воркеры

Один task = один worker:
```bash
orca agent launch --agent codex --worktree active --surface terminal \
  --prompt "<task brief>" --json
```
- read-only — текущий checkout;
- writer — separate worktree;
- writer worktree — после coordinator baseline commit; текущий dirty HEAD не содержит RW-40;
- один writer на checkout;
- handoff — handle + `handed-to-terminal`.

## 9. Метрики

- first-pass gate/review rate;
- return rate по причинам;
- duplicate test runs;
- e2e pass/skip (skip ≠ pass);
- cycle time red→accepted;
- mutation kill rate;
- live window budget (1/acceptance).

## 10. Что считается готовым

Функция готова только если:
1. контракт подтверждён;
2. red/green + targeted;
3. tests не ослаблены;
4. review + G6 parity;
5. gate;
6. PM accept;
7. для live — E3;
8. docs/tracker обновлены после accept.

Иначе — partial/deferred/blocked, не «готово».
