# RW-38 — METRICS: усиленные GUI-тесты и метрики

Дата: 2026-10-23. Бинарь: `998ac701554c669fe7113883c9408798672fdc4f2192a4330de22f378eaba005` (debug).
Источник: `waves/RW-38/metrics.json` (теперь **включает FPS/p95**), `pss.json`,
`/tmp/opencode/gui-window.png`, `.gate/worker-2026-10-24T10-22-26-400Z.json`.

## 1. Метрики приложения

| Метрика | Значение | Источник |
|---|---:|---|
| FPS чат / участники | 60 / 60 | `metrics.json.bench` |
| p95 кадра | 18 мс | `metrics.json.bench` |
| Кадров за 5 с | 300 / 300 | `metrics.json.bench` |
| `frontend_ready_ms` | 27 мс | `metrics.json.bench` |
| RSS родителя (avg) | 188 298 КиБ ≈ **184,0 МБ** | `metrics.json` |
| RSS-сумма дерева (avg) | 439 696 КиБ ≈ **429,4 МБ** | `metrics-tree.json` |
| **PSS дерева** | **253 375 КиБ ≈ 247,4 МБ** | `pss.json` |
| `WebKitWebProcess` PSS | 135,3 МБ | `pss.json` |
| Потоки (родитель/дерево) | 32 / 65 | `metrics*.json` |

`metrics.json` больше не «только RSS»: поле `bench` содержит `chat/members` с
`fps`, `p95Ms`, `frames` (парсится из `BENCH` через `--bench-log`).

## 2. UI-снимок и окно

```json
{"screenCount":4,"chatRows":100000,"memberRows":24229,"emojiCategories":10,
 "settingsToggles":38,"chatRenderedRows":30,"memberRenderedRows":24,
 "chatLoadedRows":400,"memberLoadedRows":100,
 "actions":{"screenClicks":4,"categoryClicks":true,"toggleClick":true,"listScroll":true}}
```

AT-SPI/X11-проба (`scripts/gui-window-probe.py`):

| Проверка | Факт |
|---|---|
| Кнопки рейла | 4 (Чат, Эмодзи, Участники, Настройки) |
| Категории эмодзи | 10 |
| Ячейки эмодзи (именованные list item) | 20 |
| Поиск эмодзи | есть (`entry`) |
| Тумблеры настроек | 38, состояние меняется (`true → false`) |
| Чат, list item | 22 (виртуализация) |
| Участники, list item | 24 |
| Окно | 1280×820, PNG `gui-window.png` |
| Пиксели | 3 755 уникальных цветов (non-uniform) |

## 3. Тесты / гейт

| Проверка | Результат |
|---|---|
| DOM-харнесс (`gui-spike-dom`) | **20/20** |
| targeted model+bench+dom | **91/91** |
| Rust IPC contract | **20/20** |
| Rust live launch (`GUI_SPIKE_E2E=1`) | **1/1**, 9,16 с |
| Live smoke (`gui-spike`) | **1/1** |
| AT-SPI/X11 smoke (`gui-window`) | **1/1** |
| `npm run gate:worker` | **passed_with_skips**: typecheck/ui-tsc 0, meta **65/65**, unit **651/651**, spike-rust, smoke **22/22** (0 skip); 12 skip только `keyring_contract` (live, причина в тесте; с `DISCORD_LAB_KEYRING_LIVE=1 … --ignored` — 12/12 pass) |
| Workspace | **1116 tests**: 1104 passed, 12 live-keyring ignored (см. выше), 0 failed |

## 4. Мутации (доказательство чувствительности)

| Набор | Итог |
|---|---|
| GUI live (`run-mutations-gui.py`) | **G1** memberRows→100: caught; **G2** настройки 38→37: caught; restore md5 OK |
| DOM (`W38a`) | M1 CHAT_ROWS→500: 5 fail; M3 убрать `replaceChildren`: 2 fail; M4 убрать `ResizeObserver`: 2 fail; M5 убрать `el.remove()`: 8 fail; M2 эквивалентен |
| Model (RW-37) | 12 caught, 2 эквивалентных |

## 5. Соответствие аудиту

| Слабое место | Закрыто |
|---|---|
| Нет тестов на 4 экрана/100k/24229/10/38 | ✅ DOM 20 + AT-SPI |
| Нет автопроверки виртуализации/DOM-чисел | ✅ DOM-харнесс (sizer, окно, удаление строк) |
| METRICS нет, metrics.json только RSS | ✅ `METRICS.md` + `bench` в `metrics.json` |
| GBM не валит gate | ✅ smoke обязателен при дисплее; GBM/panic → fail |
| Rust: IPC/запуск не тестируются | ✅ `ipc_contract.rs` 20 + `launch.rs` 1 |
| Мутации не покрывают все риски | ✅ GUI-мутации G1/G2 + DOM M1–M5 |
