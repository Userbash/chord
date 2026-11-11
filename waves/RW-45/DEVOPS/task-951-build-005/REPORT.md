# REPORT — TASK-951: Формирование централизованного манифеста сборки `BUILD-005` и слияние `unstable -> testing`

- **Волна / Карточка:** Wave RW-45 / `TASK-951` (`task_4eb7b589d722`, dispatch `ctx_c51a1083fc96`)
- **Отдел:** `devops`
- **Приоритет:** `P0`
- **Статус:** Выполнено (все критерии приёмки DoD 1–3 закрыты на 100%)

---

## 1. Сделано

1. **Подзадача 951.1 — Сверка и слияние рабочих веток в `unstable`:**
   - Проверен статус всех 128 задач предшествующих волн (`TASK-701`..`TASK-908`) в `waves/kanban-board.json`: `128/128` в статусе `done` (`0` незавершённых).
   - Выполнено слияние рабочей ветки `feature/sprints-1-5-build-004` (`6f60b11`) в интеграционную ветку `unstable` (`/home/sanya/orca/workspaces/project/unstable`) с флагом `--no-ff` для сохранения истории ревью (`1e7af5a926707938c14975a31099e94e4c7036dc`).
   - Проверена чистота рабочего дерева `unstable` (`git status --porcelain`).

2. **Подзадача 951.2 — Формирование манифеста сборки `waves/BUILD-005.json`:**
   - Сформирован централизованный машиночитаемый манифест `/home/sanya/orca/project/waves/BUILD-005.json` со схемой `1.0`, метаданными сборки (`build_id: "BUILD-005"`, `unstable_commit: "1e7af5a926707938c14975a31099e94e4c7036dc"`, `testing_commit: "1e7af5a926707938c14975a31099e94e4c7036dc"`, `candidate_tag: "build-005-candidate"`, `built_at: "2026-11-11T16:04:00+05:00"`), перечнем влитых коммитов (`c31aa24..1e7af5a`), контрольными суммами артефактов и полным реестром всех `128` закрытых задач (`TASK-701`..`TASK-908`) с указанием `id`, `title`, `department`, `priority`, `status: "done"`, `wave`, `closed_at`, `commit`, `summary` и `acceptance_criteria`.

3. **Подзадача 951.3 — Перенос в ветку `testing` и установка контрольного тега:**
   - В рабочем дереве ветки `testing` (`/home/sanya/orca/workspaces/project/testing`) выполнено слияние `unstable -> testing` (`c31aa24..1e7af5a`, 0 конфликтов).
   - Установлен легковесный Git-тег `build-005-candidate` на вершину ветки `testing` (`1e7af5a926707938c14975a31099e94e4c7036dc`, тип объекта `commit`).

---

## 2. Red-прогон (TDD)

Контрактный тест `waves/RW-45/DEVOPS/task-951-build-005/verify-build-005.test.mjs` запущен до слияния и генерации манифеста (`waves/RW-45/DEVOPS/task-951-build-005/red.log`):

```text
▶ TASK-951: Централизованный манифест BUILD-005 и слияние unstable -> testing
  ✖ 951.1: рабочая ветка feature/sprints-1-5-build-004 влита в unstable (--no-ff), рабочее дерево unstable чистое
  ✖ 951.2: файл waves/BUILD-005.json существует и соответствует JSON-схеме манифеста сборки
  ✖ 951.2: манифест waves/BUILD-005.json содержит полный реестр всех 128 выполненных задач
  ✖ 951.3: ветка testing синхронизирована с unstable, легковесный тег build-005-candidate указывает на вершину testing
ℹ tests 4 | pass 0 | fail 4
```

---

## 3. Green-прогон (TDD)

После выполнения слияний, генерации `waves/BUILD-005.json` и установки тега `build-005-candidate` (`waves/RW-45/DEVOPS/task-951-build-005/green.log`):

```text
▶ TASK-951: Централизованный манифест BUILD-005 и слияние unstable -> testing
  ✔ 951.1: рабочая ветка feature/sprints-1-5-build-004 влита в unstable (--no-ff), рабочее дерево unstable чистое (20.11ms)
  ✔ 951.2: файл waves/BUILD-005.json существует и соответствует JSON-схеме манифеста сборки (1.92ms)
  ✔ 951.2: манифест waves/BUILD-005.json содержит полный реестр всех 128 выполненных задач (1.79ms)
  ✔ 951.3: ветка testing синхронизирована с unstable, легковесный тег build-005-candidate указывает на вершину testing (20.81ms)
ℹ tests 4 | suites 1 | pass 4 | fail 0 | skipped 0
```

---

## 4. Targeted, Meta & Мутационный гейт (5/5)

- **Контрактный тест манифеста и веток (`verify-build-005.test.mjs`):** `4/4 pass (0 fail, 0 skip)`.
- **Мета-тесты в ветках `unstable` и `testing` (`node --test tests/meta/*.test.mjs`):** `138/138 pass (23 suites, 0 fail, 0 skip)`.
- **Мутационный гейт (5/5 мутаций на `waves/BUILD-005.json`):**
  1. `M1` (`build_id -> BUILD-004`) — **KILLED**
  2. `M2` (`total_tasks -> 127`) — **KILLED**
  3. `M3` (удаление 1 задачи из `tasks[]`) — **KILLED**
  4. `M4` (`tasks[0].status -> in_dev`) — **KILLED**
  5. `M5` (искажение `unstable_commit`) — **KILLED**
  - Результат: **5/5 убито (100% kill rate)**, исходный файл побайтно восстановлен и проверен по MD5.

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `waves/BUILD-005.json` | `3f70fbb949c631ff5f59f344dbb765ec` |
| `waves/RW-45/DEVOPS/task-951-build-005/verify-build-005.test.mjs` | `7b4250dc2ef6ef71d8f9366a90198cd4` |

---

## 6. Риски и замечания

- Рисков и конфликтов слияния нет (`0` конфликтов, `P0 = 0`, `P1 = 0`).
- Ветки `unstable` и `testing` указывают на коммит `1e7af5a926707938c14975a31099e94e4c7036dc`, помеченный легковесным тегом `build-005-candidate`.
