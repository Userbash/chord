# PARALLEL-LANES — параллельная разработка без ожидания бэкенда

Дата: 2026-10-24. PM. Заменяет последовательную схему «ждём backend → делаем UI».
Связано: `TEAMS.md`, `TASKS.md`, `STUB-IMPLEMENTATION-PLAN.md`, `QUALITY/STRATEGY.md`.

## 1. Диагноз

Сейчас фактически работает **одна группа** (G4 QA), остальные простаивают:
G1 свободен, G2 свободен, G3 свободен, G4b/G5 свободны. Это не «объединённая группа»,
а последовательная очередь. Причина — ожидание backend/live, хотя UI можно делать
против **контракта и fake-транспорта**, а дизайн/аудит вообще не зависят от кода.

## 2. Принцип: contract-first, merge-later

Любая функция сначала получает **пакет контракта**:

| Компонент | Что фиксируется |
|---|---|
| IPC | имя команды, аргументы (camelCase), результат |
| Ошибки | `kind`, `status`, `retry_after_ms`, отсутствие секретов |
| Fixture | успех/401/403/404/429/сеть, fake-ответ |
| Тест | contract/UI-тест на fake, red-first |
| Owner | G1 backend, G2 UI, G4 test |

После этого G1 и G2 работают **независимо**:
- G1 реализует команду и fake-контракт;
- G2 рисует UI и вызывает `invoke("...")` с тем же контрактом;
- G4 проверяет оба слоя;
- PM/G6 сливает и принимает.

Нельзя: придумывать поля «чтобы UI заработал»; менять контракт без ADR/PM.

## 3. Параллельные линии

| Линия | Кто | Что делает сейчас | Не зависит от |
|---|---|---|---|
| **A. Backend contracts** | G1 | guild lifecycle, READY→model, typing/presence contracts | UI/design |
| **B. Product UI** | G2 | меню сервера, enter/leave, channel switch — на fake invoke | backend live |
| **C. Design** | G3 | SERVER-MENU принят; VOICE/STREAM spec; tokens/DP-02 | кода |
| **D. QA/tooling** | G4 | CLI fixture, gate/coverage, mutation | продукта |
| **E. Platform/Security** | G5+G4b | unsafe/FFI registry, voice devices/capture feasibility | UI |
| **F. Integration** | PM+G6 | contract freeze, merge, parity, gate | — |

### WIP

- **2 кодовых линии** одновременно (A/B/C — не более двух с кодом);
- D/E — read-only/design, могут идти параллельно;
- один `cargo` на target-dir; один merge в batch;
- live-окно — отдельно.

## 4. Merge protocol

Детали окна/возврата: `MERGE-POLICY.md` (P0/P1 → return до полного цикла; merge на границе batch).

1. **Contract freeze**: `CONTRACTS/<feature>.md` + fake fixture + test.
2. G1 и G2 пишут в **своих файлах**; пересечений нет.
3. Handoff: md5, targeted-логи, known gaps.
4. PM merge: только после targeted green; чужие файлы не правятся.
5. G6 проверяет `[L]/[S]/[D]` и evidence; G4 — gate.
6. При конфликте интерфейса — PM/ADR, не «подгонка» одной стороны.

## 5. Как снижаем простой

| Ситуация | Что делает отдел |
|---|---|
| backend не готов | UI пишет против fake + contract test |
| live-окно недоступно | offline-контракт/фикстура, live помечается `[A]` |
| code-слот занят | read-only аудит/дизайн/тест-план (G3/G4b/G5) |
| ждём RE | готовим UI-состояния/empty/error, не выдумывая endpoint |
| нет данных | typed `unknown`, не фиктивная функция |

## 6. Текущее распределение (2026-10-24)

| Линия | Задача | Статус |
|---|---|---|
| D G4 | QA4-01a `cdp eval/clicktext/list/shot` | **accepted (D-90)** |
| D G4 | QA4-02 gate workspace | **in work** |
| B G2 | G-05 меню сервера на fake invoke + G-04 leave confirm | **in work** |
| C G3 | `VOICE.md` (panel/participants/devices/PTT) | **design in work** |
| E G4b/G5 | unsafe/FFI registry + voice device feasibility | **read-only audit in work** |
| A G1 | контракт G-01 READY→guilds / typing | next, contract-first |
| F PM/G6 | merge Q-TOKEN/G1-01/G-04; parity gate | active |

## 7. Что считается «готово к слиянию»

- контракт зафиксирован; fake + negative tests;
- UI не зависит от живого backend в тестах;
- G1 и G2 green по отдельности;
- G4 подтверждает targeted; PM — gate;
- G6 parity `parity-ok`/`deferred`;
- заглушка удалена из `STUB-TRACKER`.

## 8. Метрика простоя

- `idle` = отдел без active/next задачи >1 batch;
- `waiting` = задача `blocked` без конкретного контракта/окна;
- цель: 0 `idle`, ≤1 `waiting` на отдел; при `idle` PM выдаёт read-only работу.
