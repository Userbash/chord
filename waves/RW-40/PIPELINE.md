# PIPELINE — RE → PM → пул → отделы → merge

Дата: 2026-10-24. PM. Связано: `PARALLEL-LANES.md`, `MERGE-POLICY.md`,
`QUEUE.md`, `PARITY/G6-CHARTER.md`.

## 1. Цель

Убрать простой отделов и сделать поток задач непрерывным:

```
G6 RE (дамп/эталон)
  → отчёт: что реализовать, как, evidence
  → PM: triage, приоритет, владелец
  → пул задач (QUEUE/TASKS/STUB-TRACKER)
  → отдел берёт задачу, не ждёт
  → merge в integration
  → G6 проверяет паритет
  → PM accept
```

## 2. G6 — отдел обратной разработки

G6 **обязан** выдавать по каждому кандидату:

| Поле | Содержание |
|---|---|
| ID | G6/STUB/G-xx |
| Функция | что делает, для пользователя |
| Источник | `app.asar`, dump, live; `[L]/[S]/[D]` |
| Наше состояние | implemented/partial/absent |
| Gap | чего нет |
| Контракт | IPC/REST/UI/data, errors, states |
| DoD | как проверить |
| Тест | contract/UI/e2e |
| Риск | security/live/RE |
| Owner | G1/G2/G3/G4/G4b/G5 |
| Приоритет | P0–P3 + WSJF |
| Ready | можно кодировать / нужен RE/live |

Выход: `PARITY/G6-REPORT-<batch>.md` + обновление `PARITY-MATRIX`/`GAP-BACKLOG`.
Без отчёта G6 задача в пул не попадает.

## 3. PM — triage и распределение

PM за один batch:

1. Читает G6-отчёт.
2. Проверяет evidence (`[L]/[S]/[D]`, не `[M]/[A]`).
3. Ставит P0–P3 и WSJF, выбирает owner.
4. Добавляет в `QUEUE.md`/`TASKS.md`/`STUB-TRACKER`.
5. Проверяет загрузку: у каждого отдела есть active/next; idle >1 batch → read-only задача.
6. Фиксирует merge window и зависимости.

Правила:
- P0 выше P1A, P1A выше P1B, P2/P3 не вытесняют.
- 2 code slots; если заняты — отдел берёт read-only/design/contract.
- Один owner на карточку; reviewer — не автор.
- Нельзя «придумать» функцию: только G6 evidence.

## 4. Отдел — pull-модель

Когда слот освобождается, отдел **сам берёт верхнюю задачу** из своего пула:

1. `QUEUE.md` / `TASKS.md` — ready-задачи с owner.
2. Если ready нет — read-only задача (аудит/дизайн/тест-план) из своего пула.
3. Если нужен контракт — G1/G6 пишут `CONTRACTS/<feature>.md` + fake.
4. Не ждать другой отдел: UI на fake, backend на contract test.
5. После green — handoff PM, merge по `MERGE-POLICY.md`.

## 5. Merge в общую пред-ветку

- Integration-поток: `integration/rw-40` (координатор создаёт/коммитит; PM не коммитит).
- Merge только по `MERGE-POLICY.md`: targeted → batch gate → review → G6 → PM accept.
- Возврат: P0/P1/gate fail → та же карточка `returned` до полного цикла.
- После merge — новый gate; P0/P1 не переносятся в deferred.

## 6. Балансировка нагрузки

| Ситуация | Действие PM |
|---|---|
| отдел idle | выдать read-only: аудит, тест-план, дизайн, контракт |
| code slots 2/2 | отдел готовит следующий срез/контракт/fake |
| отдел ждёт backend | UI на fake + contract test |
| отдел ждёт live | offline-фикстура; live `[A]` |
| отдел перегружен P1 | P2/P3 в backlog, не парализовать поток |
| 2 одинаковых отказа | смена подхода/декомпозиция |

Цель: **0 idle >1 batch**, ≤1 waiting на отдел.

## 7. Текущий цикл (2026-10-24)

| Шаг | Кто | Состояние |
|---|---|---|
| G6 report | G6 | **new session** |
| PM triage | PM | active |
| Code #1 | G2 | DP-02 |
| Code #2 | G1 | SEC-02 |
| Design | G3 | STREAM/tokens |
| QA read-only | G4 | walk/e2e plan |
| Security/Platform | G4b/G5 | PLAT-11/voice devices |
| Merge | PM+coordinator | после gate |

## 8. Definition of Done потока

- у каждого отдела есть active или next;
- задача в пуле имеет owner/P/DoD/test/evidence;
- merge только после полного цикла;
- G6 отчёт обновлён;
- tracker/queue синхронизированы.
