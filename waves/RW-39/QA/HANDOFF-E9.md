# QA HANDOFF E9 — RW-39 (frontend+backend+ядро)

Дата: 2026-10-23. От: PM (DeepSeek). Кому: QA-отдел.
Статус: **готово к тестированию** на текущем рабочем дереве; коммит/ветку делает координатор.

## 1. Что тестировать

- `QA/PLAN-E9.md` — полная матрица кнопок и функций (N/C/M/E/U/S/K/B/P).
- Frontend: 4 экрана, чат, композер, эмодзи, участники, настройки, поиск, клавиши.
- Backend: `ready`, `messages_page`, `members_page`, `settings`, `search_page`; ошибки/лимиты.
- Ядро: `History`-пагинация (ids совпадают с BFF), `SearchFilters` Deserialize.

## 2. Артефакты

| Файл | Что |
|---|---|
| `QA/artifacts/gate-release.json` | полный гейт: passed_with_skips |
| `QA/artifacts/gui-probe.json` | AT-SPI/X11: 4 рейла, 10 категорий, 38 тумблеров, PNG/PID |
| `QA/artifacts/gui-window.png` | скриншот окна 1280×820 |
| `PM/md5-merge.txt` | md5 изменённых файлов (37) |
| `PM/window-runs.jsonl` | бюджет живых прогонов |

## 3. Результаты приёмки

- `gate:release` → **passed_with_skips**: typecheck 0, ui-tsc 0, meta 29/29, unit 298/298,
  spike-rust ok, smoke 22/22 (0 skip), coverage 298/298, golden ok.
- `e2e` 4 skip — opt-in live Discord, **не считается прохождением**.
- `cargo test --workspace` — **0 failed**.
- `cargo test` (gui-spike) — 59: 3 unit + 6 core_bridge + 29 data_contract + 20 ipc + 1 launch.
- Живая проба: 38 тумблеров, toggle `true→false`, PNG 1280×820, 5089 цветов.

## 4. Порядок QA

1. Взять дерево/ветку от координатора, зафиксировать md5.
2. Пройти `PLAN-E9` §1–§7 вручную/AT-SPI; для каждого пункта: работает / disabled с подсказкой /
   дефект с repro.
3. §8 backend: прогнать команды и негативные кейсы (лимиты 64 МиБ/256, `d:null`, битый item).
4. §9 нефункциональные: FPS/p95, PSS, cold start, консоль.
5. Отчёт `QA/REPORT-E9.md`: P0/P1/P2, repro-команда, скриншот/лог.
6. P0/P1 → возврат PM; P2 — карточка с владельцем.

## 5. Известные ограничения (не дефекты)

- Отправка сообщений/вложения/реакции — Phase 3 (backing disabled).
- QuickSwitcher/профиль/меню гильдии/треды — disabled-affordance.
- Голос/видео UI и реальная сеть Discord — отдельный сетевой отдел (NET), не в E9.
- e2e live Discord — только по согласованному окну владельца.
