# RW-37 — RUNBOOK: замер PSS после оконной модели

Цель: подтвердить, что `PageStore` снизил PSS дерева до **≤300 МБ** без потери 60 fps.

## 1. Сборка

```bash
cd rust/gui-spike
cargo build --offline
sha256sum target/debug/discord-gui-spike
```

## 2. Запуск (X11 + software-фолбэк)

```bash
cd rust/gui-spike
SPIKE_EXIT_AFTER_MS=55000 SPIKE_AUTOBENCH=1 \
  GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 \
  ./target/debug/discord-gui-spike > ../../waves/RW-37/app.log 2>&1 &
echo $! > ../../waves/RW-37/app.pid
```

Маркеры в `app.log`: `window-ready`, `UI_SNAPSHOT`, `BENCH`.

**Перед live smoke обязательно пересоберите бинарь** (`cargo build`): Tauri встраивает
`ui/*` на этапе сборки, и smoke на старом бинаре проверяет встроенную копию, а не сдаваемые
файлы. Тест `tests/smoke/gui-spike.smoke.test.mjs` падает, если бинарь старше `ui/*`.

## 3. Замер

```bash
# RAM родителя
node scripts/gui-spike-bench.mjs --pid "$(cat waves/RW-37/app.pid)" \
  --duration-ms 3000 --json waves/RW-37/metrics.json
# RAM дерева
node scripts/gui-spike-bench.mjs --pid "$(cat waves/RW-37/app.pid)" --tree \
  --duration-ms 3000 --json waves/RW-37/metrics-tree.json
# PSS дерева
python3 waves/RW-36/measure-pss.py --pid "$(cat waves/RW-37/app.pid)" \
  --json waves/RW-37/pss.json
```

## 4. Критерии приёмки

| Метрика | Цель | Где |
|---|---|---|
| PSS дерева | **≤300 МБ** | `pss.json` |
| FPS | ≥55 (факт 60) | `BENCH` в `app.log` |
| p95 кадра | ≤20 мс | `BENCH` |
| rendered-строки | ≥10 и `<` total | `UI_SNAPSHOT` |
| `lowSample`/`stalled` | отсутствуют | `BENCH` |
| Модель в памяти | `loadedRows ≤ pageSize*maxPages` | `stats()` в `UI_SNAPSHOT` (если добавлено) |

## 5. Если PSS всё ещё >300 МБ

PM возвращает W37b на переработку: проверить, что `app.js` не материализует массивы
`makeMessages(100000)`/`makeMembers(24229)` целиком; `PageStore.fetchPage` должен
генерировать только запрошенную страницу. Проверить `loadedRows` через `stats()`.
