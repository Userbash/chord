# REPORT — RW-36 / B07: GUI-спайк Tauri (P0-01, P0-06)

Статус: **принято условно** (функционально и по FPS — да; бюджет памяти — открытый
follow-up). Дата: 2026-10-23. Роли: TL (Lead), W36a (`ui/app.js`), W36b
(`scripts/gui-spike-bench.mjs` + тесты), QA-1 (мутации), REV-1 (два прохода review).

## 1. Сделано

- **Tauri 2 + статический фронтенд** в изолированном workspace `rust/gui-spike/`
  (GUI-крейты не попали в `rust/Cargo.lock` продукта).
- **4 экрана**: чат 100k, эмодзи-пикер (10 категорий), участники 24 229, настройки
  (38 тумблеров). Виртуализация списков, переключение экранов, поиск эмодзи.
- **Виртуализация починена**: `ResizeObserver` + repaint при `switchScreen` + замер
  rendered-строк после `nextFrame`. Было 8/8 строк, стало 30/24.
- **Бенчмарк прокрутки**: guard-таймаут (`stalled`), `lowSample`, `benchRunning` в
  `finally`, nearest-rank p95 как в скрипте.
- **Метрики честные**: `frontend_ready_ms` — старт скрипта (21 мс), а не конец прогона;
  RAM снимается по дереву (`--tree`) и PSS (`measure-pss.py`), а не только по родителю.
- **Валидация снимков**: `SNAPSHOT_FLOORS` (включая rendered-строки ≥10),
  `validate_bench_payload` (JSON + chat/members-объекты — защита от инъекции в лог).
- **REA**: бинарь открыт и проверен (`open_binary`: ELF64 x86_64, sha256);
  REA-провайдер `pwntools-elf` недоступен в среде (`REA_PWNTOOLS_PYTHON` не задан) —
  анализ шёл по shipped-artifact и живым замерам, без установки пакетов.
- **Документы**: `docs/rust-client/ADR-1.md` (решение по тулкиту), `waves/RW-36/METRICS.md`
  (все числа), RUNBOOK с DMA-BUF-фолбэком и `--tree`.

## 2. Red-first

- W36b: `red-import.log` (`percentile95` не экспортирован), `red.log` (4 fail на
  `--tree`), `red-mutant-hwm.log` (мутант HWM ловится).
- Rust: red-first доказан мутационным драйвером `run-mutations-rust.py` — до усиления
  тестов 7 мутантов выживали; после — `9 caught / 0 survived` (R1, R2, R3, R5, R6, R7,
  R9, R10, R11), md5 восстановлен.

## 3. Green

| Проверка | Результат |
|---|---|
| `cargo test` (spike, bin) | **16 passed / 0 failed** |
| `node --test tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-dom.test.mjs` | **40 passed / 0 failed** |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | **0 ошибок** |
| `cargo fmt -- --check` (spike) | чисто |
| `cargo clippy --offline -- -D warnings` (spike) | 0 |
| `GUI_SPIKE_E2E=1 node --test tests/smoke/gui-spike.smoke.test.mjs` | **1 passed / 0 skipped** |
| Мутации Rust | **9/9 caught, restore md5 OK** |
| `cargo test --offline --workspace` (продукт) | **1069 passed / 0 failed** |

## 4. Gate

- `npm run gate:worker` → **passed_with_skips**: typecheck 0, meta 29/29, unit 209/209,
  smoke 20/21 (1 skip — opt-in GUI live smoke; при `GUI_SPIKE_E2E=1` проходит).
- Отчёт: `.gate/worker-2026-10-23T14-17-38-564Z.json`.
- Мутации W36b (Node) — из первого прогона QA-1: 9/10 caught, 1 survived (`parseStatus`
  не покрыт); закрыто W36b табличными тестами `parseStatus`.

## 5. Метрики (финальный бинарь `2a47bd3c…`)

| Метрика | Факт | Бюджет |
|---|---|---|
| Чат 100k / участники 24 229 | 60 fps, avg 16,67 мс, p95 18 мс | FPS ≥55, p95 ≤20 мс — ✅ |
| UI-снимок | 4 экрана, 30/24 rendered, 10 категорий, 38 тумблеров | ✅ |
| Cold start (процесс → окно) | 102–137 мс | ≤1,5 с — ✅ |
| Холодная сборка | 29,56 с (261 крейт) | — |
| RAM родителя | 184,2 МБ RSS | — |
| **PSS дерева** | **393,9 МБ** (WebKit 281,6 МБ) | ≤300 МБ — ❌ |

Подробно — `METRICS.md`. Вывод: Tauri проходит по FPS/старту/функциям; память превышает
бюджет из-за материализации 124k объектов в JS — нужна оконная/постраничная модель.

## 6. Ревью

- `REVIEW-codex.md` (проход 1): FAIL, P0=0, P1=4 — все закрыты.
- `REVIEW-codex-pass2.md` (проход 2): **PASS, P0=0, P1=0**; 13 P2, из них закрыты
  критичные: перезамер на финальном бинаре, smoke ловит регрессию виртуализации,
  PSS-артефакт, `nextFrame`-таймаут, `benchRunning` в `finally`, `parse_process_metrics`
  без `unwrap_or(0)`, тест `report_bench`, RUNBOOK, wording бюджета кадра.

## 7. Файлы + md5

```
4f8611db118266731be7651076dce14c  rust/gui-spike/src/main.rs
c0330a158ffa93aee22404261b3f7871  rust/gui-spike/ui/app.js
83be4cf5ef6435e4e22447bbaf0c027a  rust/gui-spike/ui/index.html
ab900e10830fbce74dee35da868848be  rust/gui-spike/ui/style.css
54cfb3c1ccad25d951ae5861109cffd7  rust/gui-spike/Cargo.toml
eb99adb3481255a3de282853a0c55f8c  rust/gui-spike/tauri.conf.json
f1cab1e9cf69fc41c55dd1106c41ac74  rust/gui-spike/capabilities/default.json
d0dd2b8bc3f44f39651ae5fc91071b51  rust/gui-spike/ui/tsconfig.json
ebc9dea16bb81ef0f8f0a005c633f27f  rust/gui-spike/.gitignore
dd3ae8e1ca0389fbde39740cb72a5d15  scripts/gui-spike-bench.mjs
6171a64f3ec5deb0e38a9311b9f6e15c  tests/unit/gui-spike-bench.test.mjs
d7bcf9f092b5408d3fc2a2d2834bb34b  tests/unit/gui-spike-dom.test.mjs
7c1099da11e41e2c1b822d5ed8d951be  tests/smoke/gui-spike.smoke.test.mjs
c75f53ba202ba4d2105749ea92cdca63  docs/rust-client/ADR-1.md
```

## 8. Риски и follow-up

1. **P2 (архитектура)**: PSS 394 МБ > 300 МБ. Карточка P3 (UI-слой): оконная/постраничная
   модель данных (webview держит ~200–500 строк, Rust отдаёт страницы) — до объявления
   памяти «зелёной». Бюджет §9 пересчитать после неё.
2. **P2**: `ui/tsconfig.json` не подключён к `npm run typecheck` (гейт гоняет только
   `tsconfig.json`/`tsconfig.agent.json`). Подключение — правка `package.json`, требует
   разрешения владельца; сейчас UI-tsc гоняется вручную в отчёте.
3. **P2**: release-профиль не измерялся (debug: 29,56 с сборка, 102–137 мс старт,
   PSS 394 МБ). Перед релизом — повтор на `--release`.
4. **P3**: ширина p95-различия UI/скрипт устранена формулой; `nextFrame` теперь с таймаутом;
   smoke не проверяет `lowSample`/`stalled` явно (они не появляются в живом прогоне).
5. **Среда**: без `WEBKIT_DISABLE_DMABUF_RENDERER=1` + `LIBGL_ALWAYS_SOFTWARE=1` рендер
   ломается (GBM), rAF не тикает. Это X11/software-фолбэк, не дефект спайка.
6. **REA**: `pwntools-elf`-провайдер недоступен (нет `REA_PWNTOOLS_PYTHON`); статический
   layout-анализ бинаря не выполнялся. Для RE-задач волны — либо задать Python-среду,
   либо продолжать по shipped-artifact/живым замерам.
