# TASK — task-287 (`G6 Docs L1 / P0`, `DOCS-FIX-01`: Синхронизация `TDD-PLAN.md` §8 с 66 новыми `tests/unit/*.test.mjs`)

- **Отдел:** G6 Tech Writer / Parity (`DeepSeek` — `--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Уровень сложности / Приоритет:** `L1 / P0` (блокер `tests/meta/plan-sync.test.mjs` в `gate:worker`, по `waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md`)
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/docs-task-287-tdd-plan-sync` (`branch:docs/task-287-tdd-plan-sync`)
- **Разрешённый к изменению файл (строго 1 файл в worktree):**
  - `TDD-PLAN.md` (§8 таблица «Тест | Модуль | Экспорты / контракт», строки после строки 409)
- **Артефакты отчёта:**
  - `/home/sanya/orca/project/waves/RW-40/G6/task-287-tdd-plan-sync/REPORT.md`
  - `/home/sanya/orca/project/waves/RW-40/G6/task-287-tdd-plan-sync/red.log`
  - `/home/sanya/orca/project/waves/RW-40/G6/task-287-tdd-plan-sync/green.log`

## Точное описание задачи (без поиска по репозиторию)

1. Перейди в `/home/sanya/orca/workspaces/project/docs-task-287-tdd-plan-sync`.
2. **Red-прогон:** запусти `node --test tests/meta/plan-sync.test.mjs` и сохрани вывод в `/home/sanya/orca/project/waves/RW-40/G6/task-287-tdd-plan-sync/red.log`.
   Тест падает на `каждый тестовый файл (unit/meta/smoke/e2e) описан в §8 ровно один раз` со списком из 66 файлов `tests/unit/*.test.mjs`, отсутствующих в таблице `TDD-PLAN.md` §8.
3. **Как устроен парсер `tests/meta/plan-sync.test.mjs` (строки 52–72, 118–150):**
   - Каждая строка таблицы §8 имеет вид:
     `| \`tests/unit/<name>.test.mjs\` | \`rust/gui-spike/ui/features/<name>.js\` | \`token1\`, \`token2\` |`
   - Во 2-й колонке указывай соответствующий UI-модуль `\`rust/gui-spike/ui/features/<name>.js\`` (или `—`, если прямой пары нет). Так как путь начинается с `rust/`, парсер `parsePlanRows` фильтрует только `src/` и `bin/` для проверки `src`-экспортов, поэтому `rust/gui-spike/ui/features/...` безопасно документирует связь.
   - В 3-й колонке укажи 2–4 идентификатора в обратных кавычках (например, экспортируемые функции/константы, которые **гарантированно буквально встречаются** в тексте данного тестового файла `tests/unit/<name>.test.mjs`, т.к. тест на строке 133 проверяет `text.includes(token)`!).
4. Добавь в `TDD-PLAN.md` (§8, сразу после строки 409 `| \`tests/unit/server-menu.test.mjs\` | ... |`) ровно 66 недостающих строк для всех 66 файлов `tests/unit/*.test.mjs`, предварительно проверив реальные экспортируемые/импортируемые токены в каждом из этих 66 тестовых файлов (можно однострочным `node -e` скриптом прочитать импорты из каждого из 66 файлов и сформировать строки таблицы!).
5. **Green-прогон:**
   - Запусти `node --test tests/meta/plan-sync.test.mjs` (`5/5 PASS`) и сохрани вывод в `/home/sanya/orca/project/waves/RW-40/G6/task-287-tdd-plan-sync/green.log`.
   - Запусти `npm run test:meta` и убедись, что мета-тесты проходят.
6. Запиши `/home/sanya/orca/project/waves/RW-40/G6/task-287-tdd-plan-sync/REPORT.md` (с `md5sum TDD-PLAN.md`) и заверши задачу через `orca orchestration complete`. Не делай `git commit/push`.
