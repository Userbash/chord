# REPORT — RW-38: усиление тестов GUI (DOM, Rust/Tauri IPC, AT-SPI-окно, gate)

Дата: 2026-10-23. PM: DeepSeek. Воркеры: W38a (DOM), W38b (Rust), PM (AT-SPI, gate, metrics).
Продуктовые фичи не добавлялись — только тесты, доступность, наблюдаемость и качество.

**Статус: принято.** REV-1 pass2 — **PASS (P0/P1=0)**; P1 pass1 закрыты, P2 (8) разобраны
или вынесены в follow-up. Финальный гейт `passed_with_skips` — единственные 12 skip —
live-keyring (причина в `#[ignore]`, live-прогон 12/12 pass).

## 1. Что закрыто из аудита

| Слабое место | Решение |
|---|---|
| Нет тестов на 4 экрана/100k/24229/10/38 | `gui-spike-dom.test.mjs` (20 кейсов) + AT-SPI-проба |
| Нет автопроверки виртуализации/DOM-чисел | DOM-харнесс: sizer 100000×44, окно, удаление строк, `loadedRows≤500` |
| `metrics.json` только RSS; METRICS нет | `--bench-log` → `bench` в JSON; `METRICS.md` с FPS/p95/PSS |
| GBM не валит gate | live smoke обязателен при дисплее; GBM/Gdk/panic → fail |
| Rust IPC/запуск не тестируются | `src/lib.rs`, `ipc_contract.rs` (20), `launch.rs` (1, live) |
| Мутации не покрывают все риски | GUI-мутации G1/G2 + DOM M1–M5 + model 12 |

## 2. Новые тестовые слои

### DOM (W38a)
- `tests/support/gui-dom-harness.mjs` — DOM-шим без npm-зависимостей: селекторы,
  classList/dataset/attributes, `HTMLInputElement`/`HTMLButtonElement`, `ResizeObserver`,
  очередь `requestAnimationFrame`, `window.__TAURI__` с журналом `invoke`.
- `tests/unit/gui-spike-dom.test.mjs` — **20 кейсов**: 4 экрана, 100k/24229, 10 вкладок,
  38 тумблеров, виртуализация/удаление строк, `UI_SNAPSHOT`, отсутствие дублей, границы.

### Rust (W38b)
- `src/lib.rs` — чистая логика (snapshot/bench/process metrics), `main.rs` — тонкие
  `#[tauri::command]`-обёртки.
- `tests/ipc_contract.rs` — **20 кейсов** IPC-контракта.
- `tests/launch.rs` — live-запуск `CARGO_BIN_EXE`, `window-ready` + `UI_SNAPSHOT`,
  отсутствие GBM/panic; opt-in `GUI_SPIKE_E2E=1`, skip-aware без дисплея.

### AT-SPI + X11 (PM)
- `scripts/gui-window-probe.py` — находит окно, обходит accessibility-дерево, кликает
  кнопки/тумблеры, проверяет 4 рейла/10 категорий/38 тумблеров/22–24 list item,
  снимает PNG через `XGetImage` (1280×820, 3 755 цветов), проверяет отсутствие GBM/panic.
- ARIA в `ui/index.html`/`ui/app.js`: `role="list"/"listitem"/"tab"/"gridcell"`,
  `aria-label` строкам/рейлу/экранам, `aria-selected` вкладкам, `aria-current` рейлу.
- `tests/smoke/gui-window.smoke.test.mjs` — обёртка (skip только без дисплея).

### Gate / metrics (PM)
- `scripts/gui-spike-bench.mjs`: `--bench-log` + `parseBenchLog` (FPS/p95 в `metrics.json`),
  **45/45** тестов.
- `tests/smoke/gui-spike.smoke.test.mjs`: авто-запуск при дисплее, GBM/panic → fail,
  stale binary → fail.
- `scripts/gate.mjs`: `--test-concurrency=1` для smoke (два GUI-окна не конфликтуют).

## 3. Проверки

| Проверка | Результат |
|---|---|
| `npm run gate:worker` | **passed_with_skips**: typecheck/ui-tsc 0, meta 65/65, unit 651/651, spike-rust, smoke 22/22 (0 skip); 12 skip — keyring live (причина в тесте) |
| DOM | 20/20 |
| targeted model+bench+dom | 91/91 |
| Rust IPC + launch | 20/20 + 1/1 (9,16 с) |
| AT-SPI-проба | ok: 4/10/38, 22/24 list item, PNG non-uniform |
| Workspace | 1116 tests: 1104 passed, 12 live-keyring ignored, 0 failed |
| clippy/fmt (spike) | 0 / OK |

Мутации: GUI **G1** (memberRows→100) и **G2** (38→37) — caught; DOM M1/M3/M4/M5 — caught,
M2 эквивалентен; restore md5 OK.

## 4. Файлы + md5 (срез на 2026-10-24; UI далее меняется Phase 2 RW-39)

```
44a22d107369236face59fe6ffb60e83  rust/gui-spike/src/lib.rs
c003a9e597eea659beeff5d52e94768c  rust/gui-spike/src/main.rs
d63184cfc0057c8ff87169756c98b954  rust/gui-spike/Cargo.toml
6e2fe83e366aa7ec3bfafd114f381085  rust/gui-spike/tests/ipc_contract.rs
0f710cd44d08ae5c92ade1fdac5a226b  rust/gui-spike/tests/launch.rs
8e26df0623a00504fc92e3ed7b01ead8  tests/support/gui-dom-harness.mjs
c7eb98c3415372b85c5a0b90f2ab4174  tests/unit/gui-spike-dom.test.mjs
6ca4a51368f4fe3349ca974a3fa941d0  scripts/gui-window-probe.py
8678287b0781531f8fdfea49c3f31029  tests/smoke/gui-window.smoke.test.mjs
417bd6687f54a2063232390e579ffe39  rust/gui-spike/ui/index.html
78039af36b09fa94e5d92b207f5ce977  rust/gui-spike/ui/app.js
b2211ff74a4644ad55c51e5c8a10870d  scripts/gui-spike-bench.mjs
c8a4875ed7d7a64d9dad3a215cad9a8f  scripts/gate.mjs
```

UI-файлы (`index.html`, `app.js`, `ui/features/*`, `ui/components/*`) изменяются Phase 2
RW-39; для приёмки RW-38 актуален бинарь и зелёный гейт, а не замороженный md5 разметки.

Бинарь: `998ac701554c669fe7113883c9408798672fdc4f2192a4330de22f378eaba005`.

## 4a. P1 rework по REV-1 (2026-10-23)

REV-1 дал FAIL (P0=0, P1=2); исправлено:

1. **Ложный pass на чужом окне.** Спайк принимает `SPIKE_WINDOW_TITLE` и ставит уникальный
   заголовок (`main.rs`). Проба ищет окно **только по своему уникальному заголовку** и
   сверяет `Atspi.get_process_id()` с PID запущенного процесса. Старый экземпляр больше
   не может пройти как «наш».
2. **Эмодзи-сетка невидима в AT-SPI.** `role="grid"` без `row` заменён на `role="list"`,
   ячейки — `listitem`; проба требует именованные ячейки (порог ≥10; факт 20).
3. **P2-фиксы:** liveness-проверка (процесс жив до конца), `XDestroyImage`/`XCloseDisplay`,
   удержание X-error-handler, уникальный лог на прогон, буфер `SPIKE_EXIT_AFTER_MS`,
   kill child на таймауте, отсутствие бинаря при дисплее → fail, `--bench-log` без `BENCH`
   → exit 1, `aria-live` снят с FPS-метрик, `ui-tsc` и `spike-rust` добавлены в gate,
   smoke последовательный.

Новые метрики: PSS **247,4 МБ** (было 278,1), FPS 60/p95 18, gate **passed_with_skips**
(12 keyring live; на дисплее GUI-слой 0 skip).

## 5. Риски / ограничения

1. **DOM-шим** не эмулирует layout/CSS, всплытие событий, `>`, `+`, `~`, псевдоклассы,
   реальные таймеры, Tauri/WebKit/GBM/AT-SPI. Это быстрый слой; live-слой — Rust launch +
   AT-SPI + smoke.
2. **AT-SPI** зависит от `GTK_MODULES=gail:atk-bridge`; без шины/дисплея — skip (exit 2).
   Проверен один вьюпорт 1280×820.
3. **X11-захват** работает для XWayland-окна через `XGetImage`; root под композитором пуст.
   На чистом Wayland без XWayland нужен другой путь (KWin screencopy) — follow-up.
4. **Release-профиль** не измерялся (debug).
5. **UI-tsc** не в `npm run typecheck` — правка `package.json` требует разрешения владельца.
6. `waves/RW-38/mutations-gui.log` — live-мутации с rebuild; на медленной машине могут быть
   медленными (2 × build+probe).
7. Коммитов нет — по процессу их делает координатор.
