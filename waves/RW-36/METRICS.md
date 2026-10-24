# RW-36 / B07 — METRICS: GUI-спайк Tauri (P0-01, P0-06)

Снято на живой сессии 2026-10-23 на **финальном бинаре** `2a47bd3c…` (после всех правок).
Источник истины по контракту — `TZ.md` §3, бюджеты — `docs/rust-client/ARCHITECTURE.md` §9.
Артефакты: `app.log`, `metrics.json`, `metrics-tree.json`, `pss.json`; воспроизведение —
`remeasure.py`.

## Окружение

| Параметр | Значение |
|---|---|
| CPU | AMD Ryzen 7 7800X3D (8C/16T) |
| RAM | 45 GiB |
| Ядро | 7.3.0-4-cachyos |
| Графика | `DISPLAY=:0` (X11; Wayland-композитор без screencopy) |
| WebKitGTK | 2.54.1 |
| GTK | 3.24.52 |
| Rust | rustc 1.99.0 (b940084d7 2026-09-28) |
| Tauri | 2.12.5 |
| Окно | 1280×820, `resizable: true` |
| Бинарь (debug) | sha256 `2a47bd3ca7ac9555b2b5653d94e0ea470f0b7a24d3d982fa85ab91a87a39598f` |

## Как снималось

```bash
python3 waves/RW-36/remeasure.py          # запуск + ожидание BENCH + RAM/PSS + app.log
```

Эквивалентные команды вручную:

```bash
cd rust/gui-spike
SPIKE_EXIT_AFTER_MS=55000 SPIKE_AUTOBENCH=1 \
  GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  ./target/debug/discord-gui-spike > ../../waves/RW-36/app.log 2>&1 &

node scripts/gui-spike-bench.mjs --pid <PID> --duration-ms 3000 --interval-ms 200 \
  --json waves/RW-36/metrics.json
node scripts/gui-spike-bench.mjs --pid <PID> --tree --duration-ms 3000 --interval-ms 200 \
  --json waves/RW-36/metrics-tree.json
python3 waves/RW-36/measure-pss.py --pid <PID> --json waves/RW-36/pss.json
```

**Критично для воспроизводимости:** без `WEBKIT_DISABLE_DMABUF_RENDERER=1` и
`LIBGL_ALWAYS_SOFTWARE=1` приложение печатает `window-ready`, но валит
`Failed to create GBM buffer of size 1280x820`; `requestAnimationFrame` не тикает,
`BENCH` не появляется. Это среда (X11 + программный рендер), а не код спайка.

## UI-снимок (`UI_SNAPSHOT`)

```json
{"actions":{"categoryClicks":true,"listScroll":true,"screenClicks":4,"toggleClick":true},
 "chatRenderedRows":30,"chatRows":100000,"emojiCategories":10,
 "memberRenderedRows":24,"memberRows":24229,"screenCount":4,"settingsToggles":38}
```

| Проверка | Факт | Вывод |
|---|---|---|
| Экраны | 4 | ✅ |
| Чат | 100 000 строк, отрисовано 30 | ✅ виртуализация |
| Участники | 24 229, отрисовано 24 | ✅ виртуализация (после repaint при показе) |
| Эмодзи | 10 категорий, переключение | ✅ |
| Настройки | 38 тумблеров, клик меняет состояние | ✅ |
| Клики по экранам | 4/4 | ✅ |

Rust-валидатор `SNAPSHOT_FLOORS` требует `chatRenderedRows`/`memberRenderedRows` ≥ 10:
регрессия «нарисован только overscan» больше не проходит приёмку.

## FPS / кадр (`BENCH`)

```json
{"frontend_ready_ms":21,
 "chat":{"frames":300,"avgMs":16.67,"p95Ms":18,"fps":60},
 "members":{"frames":299,"avgMs":16.67,"p95Ms":18,"fps":60}}
```

| Сценарий | FPS | avg кадра | p95 кадра |
|---|---:|---:|---:|
| Чат 100k, прокрутка 5 с | **60** | 16.67 мс | 18 мс |
| Участники 24 229, прокрутка 5 с | **60** | 16.67 мс | 18 мс |

`frontend_ready_ms` — момент старта скрипта (DOM разобран), а не конец прогона.
`lowSample`/`stalled` в JSON отсутствуют — выборка полная, зависаний не было.

## Память

| Метрика | Значение | Комментарий |
|---|---:|---|
| RSS основного процесса (avg/p95) | 188 612 / 188 624 КиБ ≈ **184,2 МБ** | `metrics.json`, 16 сэмплов |
| HWM основного процесса | 188 640 КиБ ≈ 184,2 МБ | |
| Потоки основного процесса (max) | 32 | |
| RSS-сумма дерева (avg/p95) | 592 370 / 592 408 КиБ ≈ **578,5 МБ** | `metrics-tree.json`, 3 процесса |
| HWM-max дерева | 338 060 КиБ ≈ 330 МБ | max по процессам, не сумма |
| Потоки дерева (max) | 57 | |
| **PSS дерева** | **403 328 КиБ ≈ 393,9 МБ** | `pss.json` |

PSS по процессам дерева (`/proc/<pid>/smaps_rollup`, `measure-pss.py`):

| Процесс | RSS | PSS |
|---|---:|---:|
| `discord-gui-spike` (main) | 184,2 МБ | 80,8 МБ |
| `WebKitWebProcess` | 330,1 МБ | **281,6 МБ** |
| `WebKitNetworkProcess` | 64,2 МБ | 31,4 МБ |

**Честный footprint — PSS ≈ 394 МБ** (RSS-сумма 578 МБ завышена: shared-библиотеки
считаются в каждом процессе). Основной потребитель — `WebKitWebProcess` (JS-heap с
124 229 материализованными объектами: 100k сообщений + 24 229 участников).

## Сборка

| Режим | Время | Комментарий |
|---|---|---|
| Инкрементальная (только крейт) | ~1,2 с | `cargo build` после правки `app.js`/`main.rs` |
| Холодная (пустой target-dir) | **29,56 с** | 261 крейт с нуля, target 2,2 ГБ |

## Холодный старт

Замер: `subprocess.Popen` → первая строка `discord-gui-spike: window-ready`.

| Запуск | Время |
|---|---:|
| Первый (run1) | **137 мс** |
| Второй, disk-кэш есть (run2 warm) | **102 мс** |
| Скрипт → готовность (`frontend_ready_ms`) | 22 мс |

## Вердикт против бюджетов ARCHITECTURE §9

| Бюджет | Цель | Факт | Итог |
|---|---|---|---|
| Скролл 24k | 60 fps, ≤16 мс кадр | 60 fps, avg 16,67 мс, p95 18 мс | ⚠️ 60 fps есть; ≤16 мс на 60-Гц дисплее недостижимы (16,67 мс = период) |
| Память в простое | ≤300 МБ | PSS дерева 394 МБ | ❌ при честном замере |
| Холодный старт до READY | ≤1,5 с | 102–137 мс | ✅ |

Критерии приёмки `TZ.md` §4 (FPS ≥ 55, p95 ≤ 20 мс) **выполнены**. Бюджет памяти
ARCHITECTURE §9 — **нет**, если держать все 124k объектов в JS; это архитектурный вывод
для ADR-1, а не дефект спайка. Холодная сборка 29,56 с — приемлемо для debug-профиля
(релиз с `lto` будет дольше; в ADR фиксируется как ориентир).
