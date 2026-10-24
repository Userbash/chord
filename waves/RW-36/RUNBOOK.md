# RW-36 — runbook спайка (P0-01/P0-06)

Локально, Wayland (`wayland-1`, `DISPLAY=:0`), WebKitGTK 2.54.1, GTK 3.24.52.
Никаких сетевых вызовов во время прогона: фронтенд статический, данные генерируются в JS.

## Сборка

```bash
cd rust/gui-spike
/usr/bin/time -f 'build_wall=%e s' cargo build 2>&1 | tee ../../waves/RW-36/build.log
```

Бинарник: `rust/gui-spike/target/debug/discord-gui-spike`.

## Запуск (отдельная сессия)

```bash
cd rust/gui-spike
SPIKE_EXIT_AFTER_MS=60000 SPIKE_AUTOBENCH=1 \
  GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  ./target/debug/discord-gui-spike \
  > ../../waves/RW-36/app.log 2>&1 &
echo $! > ../../waves/RW-36/app.pid
```

**Обязательно** `WEBKIT_DISABLE_DMABUF_RENDERER=1` и `LIBGL_ALWAYS_SOFTWARE=1` на X11:
без них WebKit валит `Failed to create GBM buffer`, `requestAnimationFrame` не тикает и
`BENCH` не появляется. Маркер готовности — `discord-gui-spike: window-ready` в `app.log`.
Завершение: `kill "$(cat ../../waves/RW-36/app.pid)"` (или авто-выход по `SPIKE_EXIT_AFTER_MS`).

Wayland-фолбэк: `GDK_BACKEND=wayland` (если композитор поддерживает screencopy).

## Скриншот

```bash
grim -o "$(grim -g '' 2>/dev/null | head -1)" waves/RW-36/shot.png   # либо:
grim waves/RW-36/shot.png
```

Скриншот — доказательство, что окно живо и экраны переключаются; в git не кладём,
путь фиксируется в отчёте.

## Замер памяти

```bash
# основной процесс
node scripts/gui-spike-bench.mjs --pid "$(cat waves/RW-36/app.pid)" \
  --duration-ms 5000 --json waves/RW-36/metrics.json
# дерево процессов: main + WebKitNetworkProcess + WebKitWebProcess
node scripts/gui-spike-bench.mjs --pid "$(cat waves/RW-36/app.pid)" --tree \
  --duration-ms 3000 --json waves/RW-36/metrics-tree.json
# PSS дерева (честный footprint; RSS-сумма завышает shared-память)
python3 waves/RW-36/measure-pss.py --pid "$(cat waves/RW-36/app.pid)" \
  --json waves/RW-36/pss.json
```

Бюджет ARCHITECTURE §9 (`≤300 МБ`) сверяется по **PSS дерева**, не по RSS родителя.

## Замер FPS

1. Открыть экран «Чат» → кнопка «Замер 5 с» (прокрутка 100k).
2. Открыть «Участники» → «Замер 5 с» (24 229).
3. Значения FPS/p95 из шапки и консоли (`[bench]`) записать в `waves/RW-36/METRICS.md`.

## Критерии

- FPS ≥ 55, p95 кадра ≤ 20 мс на обоих списках; `lowSample`/`stalled` в JSON быть не должно.
- `UI_SNAPSHOT`: `chatRenderedRows` и `memberRenderedRows` ≥ 10 (иначе виртуализация
  нарисовала только overscan — регрессия P1-1).
- RAM в простое: PSS дерева ≤ 300 МБ (бюджет ARCHITECTURE §9); RSS родителя — справочно.
- Окно открывается без падений; переключение 4 экранов не роняет процесс.
- Cold start (процесс → `window-ready`) ≤ 1,5 с; холодная сборка зафиксирована.
