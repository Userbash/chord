# RW-38 — TZ: усиление тестов GUI (DOM, Rust/Tauri IPC, AT-SPI-окно, gate)

Статус: работа. PM: DeepSeek. Проектные фичи **не добавляются** — только тесты, доступность,
наблюдаемость и качество. Основание: аудит слабых мест RW-36/37.

## 1. Проблемы, которые закрываем

1. Нет тестов на 4 экрана, 100k сообщений, 24 229 участников, 10 категорий, 38 настроек.
2. Нет автопроверки виртуализации и фактического числа DOM-элементов.
3. `metrics.json` содержит только RSS; FPS/p95 — только в `app.log`/METRICS.
4. GBM-ошибки в `app.log` не валят gate.
5. Rust проверен unit-тестами, но IPC и запуск Tauri не тестируются.
6. Мутации подтверждают выбранные мутанты, но не все риски.

## 2. DoD

1. `tests/support/gui-dom-harness.mjs` — минимальный DOM-шим без npm-зависимостей;
   `tests/unit/gui-spike-dom.test.mjs` реально импортирует `ui/app.js` и проверяет:
   4 экрана, 100k/24 229/10/38, виртуализацию (DOM-строки << total, границы окна),
   `PageStore.loadedRows ≤500`, переключение экранов/категорий/тумблеров, `UI_SNAPSHOT`
   (все поля + actions), отсутствие дублей DOM.
2. Rust: `src/lib.rs` с чистыми функциями (validate snapshot/bench, parse metrics);
   `tests/ipc_contract.rs` — контракт IPC-команд (Ok/Err, границы, типы);
   `tests/launch.rs` — запуск реального бинаря (opt-in `GUI_SPIKE_E2E=1`): `window-ready`,
   `UI_SNAPSHOT`, нет GBM/panic; skip-aware без дисплея.
3. AT-SPI GUI-проба: `scripts/gui-window-probe.py` — находит окно, снимает PNG через
   GdkX11, обходит accessibility-дерево, проверяет 4 кнопки рейла, экраны, списки,
   категории, тумблеры; выполняет клики через `Atspi.Action`; JSON + exit 0/1/2.
4. ARIA/доступность в `ui/index.html`/`ui/app.js`: роли/имена для рейла, экранов, списков,
   строк, вкладок, тумблеров; без изменения визуала и производительности (только rendered-строки).
5. Gate: при наличии дисплея live smoke выполняется в `gate:worker`; GBM/panic → fail.
   `metrics.json` получает `fps`/`p95_ms` из `BENCH` (`--bench-log`).
6. Качество: `clippy -D warnings` 0, `fmt` 0, `tsc` 0, все тесты зелёные; новые мутанты
   по DOM/AT-SPI-инвариантам разобраны.

## 3. Файловая матрица

| Владелец | Файлы |
|---|---|
| W38a (DOM) | `tests/support/gui-dom-harness.mjs`, `tests/unit/gui-spike-dom.test.mjs` |
| W38b (Rust) | `rust/gui-spike/src/lib.rs`, `rust/gui-spike/src/main.rs`, `rust/gui-spike/Cargo.toml`, `rust/gui-spike/tests/*.rs` |
| W38c (AT-SPI) | `scripts/gui-window-probe.py`, `tests/smoke/gui-window.smoke.test.mjs`, `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/app.js` (только ARIA/роли) |
| PM | `scripts/gui-spike-bench.mjs`, `tests/unit/gui-spike-bench.test.mjs`, `tests/smoke/gui-spike.smoke.test.mjs`, `waves/RW-38/*`, `METRICS.md` |

`rust/gui-spike/ui/model.js` не меняется. `waves/RW-36/37/*` — read-only.

## 4. Инварианты для тестов

- `chatRows=100000`, `memberRows=24229`, `emojiCategories=10`, `settingsToggles=38`,
  `screenCount=4`.
- `chatRenderedRows`/`memberRenderedRows` ≥10 и `< total`; DOM-строки ≤ `pageSize*maxPages`
  (500) + запас окна; после скролла старые строки удаляются (нет роста DOM).
- `loadedRows ≤500`; `sizer` высота = `total*rowHeight`.
- `actions`: `screenClicks=4`, `categoryClicks=true`, `toggleClick=true`, `listScroll=true`.
- AT-SPI: 4 кнопки рейла; после клика меняется активный экран; списки/вкладки/тумблеры
  доступны как элементы с именами; PNG не одноцветный.
- GBM (`Failed to create GBM buffer`), `Gdk-Message:`, `panic`, `segmentation fault`
  в stderr → тест/гейт failed.

## 5. Приёмка

- `node --test tests/unit/gui-spike-dom.test.mjs` — зелёный, ≥15 кейсов.
- `cargo test --manifest-path rust/gui-spike/Cargo.toml` — unit + ipc_contract; `launch`
  отдельно с `GUI_SPIKE_E2E=1`.
- `python3 scripts/gui-window-probe.py --json out.json` — exit 0 на живой сессии.
- `npm run gate:worker` — passed; при дисплее GUI-smoke входит в smoke-сьют.
- Ревью: P0/P1=0; мутации по новым инвариантам.
