# RW-36 / B07 — TZ: GUI-спайк Tauri (P0-01, P0-06)

Статус: red-фаза. Решение владельца 2026-10-23: **Tauri 2 + статический фронтенд**
(без Vite/Node-бандлера). ADR-1 фиксируется по метрикам этого спайка.

## 1. Карточки

| Карточка | DoD | Тест | Зависимости |
|---|---|---|---|
| P0-01 | Окно Tauri: чат 100k, эмодзи-пикер (10 категорий), настройки (38 тумблеров); ADR-1 с FPS/RAM/сборкой | T-UI-01 | — |
| P0-06 | Список 24 229 участников: виртуализация, замер 60 fps | T-UI-02 | P0-01 |

Спайк живёт отдельно: `rust/gui-spike/` — собственный workspace, чтобы тяжёлые GUI-крейты
не попадали в `rust/Cargo.lock` продукта до решения по ADR-1.

## 2. Файловая матрица

| Владелец | Файлы |
|---|---|
| TL (Lead) | `rust/gui-spike/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/*`, `src/main.rs`, `ui/index.html`, `ui/style.css`, `waves/RW-36/*`, ADR-1 |
| W36a | `ui/app.js` — только секции виртуализации и бенчмарка (после интеграции) |
| W36b | `scripts/gui-spike-bench.mjs`, `tests/unit/gui-spike-bench.test.mjs` |
| QA-1 | `waves/RW-36/mutations*`, прогон мутаций |
| REV-1 | `waves/RW-36/REVIEW*.md` (codex) |

Общие файлы продукта (`rust/Cargo.toml`, `docs/rust-client/ADR-1.md`, `STATUS.md`, `TASKS.md`,
`waves/STATE.md`, `waves/db.jsonl`) правит только Lead после accepted.

## 3. Контракт замера

`scripts/gui-spike-bench.mjs`:

```
node scripts/gui-spike-bench.mjs --pid <PID> [--duration-ms 5000] [--interval-ms 200] [--json out.json]
```

- читает `/proc/<pid>/status` (`VmRSS`, `VmHWM`, `Threads`), не пишет в процесс;
- флаг `--tree` суммирует дерево процессов (main + WebKitNetworkProcess + WebKitWebProcess):
  `rss_kb` — сумма по процессам, `hwm_kb` — максимум (пики разных процессов не складываются),
  `threads_max` — сумма; в JSON добавляются `processes` (max за замер) и `skipped`
  (неудачные/неполные чтения `/proc`, best-effort). Без `--tree` поля отсутствуют;
- при недоступном `/proc` — exit 2 и понятное сообщение (skip-aware);
- выдаёт в stdout JSON: `{ pid, samples, rss_kb: {min,max,avg,p95}, hwm_kb, threads_max, duration_ms }`
  (+ `processes`, `skipped` в режиме `--tree`);
- `--json` дополнительно пишет файл; ошибки аргументов — exit 1.
- `percentile95(sorted)` экспортируется и используется как единый nearest-rank
  (`ceil(0.95*n)-1`); UI-бенч в `ui/app.js` считает ту же формулу.

Тест `tests/unit/gui-spike-bench.test.mjs` проверяет чистую функцию `summarize(samples)`
на синтетике (не читая реальный `/proc`), включая пустой вход и p95.

## 4. Критерии приёмки

1. `cargo build` спайка зелёный; окно запускается в отдельной сессии (X11/Wayland).
2. Три экрана + участники 24k открываются, прокрутка не падает.
3. Замер: FPS ≥ 55 на 100k/24k при прокрутке, p95 кадра ≤ 20 мс; RAM (`--tree`, PSS — ручной
   замер дерева), время сборки и холодного старта зафиксированы в `METRICS.md`.
4. `scripts/gui-spike-bench.mjs` проходит unit-тест и снимает метрики с живого PID.
5. ADR-1 обновлён числами; решение: Tauri проходит / не проходит.
6. Ревью codex: P0/P1 = 0; P2 закрыты или осознанно отложены с записью.
7. Основной workspace не сломан: `npm run gate:worker` + `cd rust && cargo test --offline --workspace`.

## 5. Вне scope

- Продуктовый UI (P3) и подключение к реальному Discord.
- Живой Discord, Frida, CDP — не трогаются в этой волне.
- Новые npm-зависимости: фронтенд статический, сборщика нет.
