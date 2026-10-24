# W38f — отчёт: снятие P1 RW-38 (P1-1, P1-2, P2-1, P2-3)

- **batch/карточка:** RW-39 / W38f (fix-итерация 2 по `waves/RW-38/REVIEW-codex.md`).
- **вердикт:** DONE — все 4 пункта red → green, `npm run gate:worker` = `passed` (smoke 22/22, skip 0).

## Сделано

### P1-1. PID-привязка пробы (`scripts/gui-window-probe.py`)

- AT-SPI: `atspi_app(Atspi, pid)` выбирает приложение **только** по `get_process_id() == pid`
  (имя больше не фильтр); ≥2 совпадения → `Fail`, 0 → `EnvFail` («нет приложения с PID …»).
  Все повторные выборки после кликов идут через `own_app()` — тоже по PID.
- X11: `x11_capture(..., pid)` читает `_NET_WM_PID` через `XGetWindowProperty`; окно берётся
  только при `title == уникальный` **и** `_NET_WM_PID == pid`. 0 → `EnvFail` («своё окно … не
  найдено»), ≥2 → `Fail`; PID пишется в `window.pid`.
- `pid` пишется в `--json` всегда (успех, `Fail`, `EnvFail`), вместе с путём лога.
- `require_alive(proc, step)` (`proc.poll()`) перед каждым шагом; child гасится в `finally`
  (и в `launch()` при ошибке — было, сохранено).
- Устранена гонка старта: проба ждёт `UI_SNAPSHOT` в логе (стартовый автоснимок сам кликает
  экраны) — иначе первый клик мог быть перебит и «активным» оказывался чужой экран.

### P1-2. Доступная эмодзи-сетка

- `ui/index.html`: `#emoji-grid` → `role="listbox"` (визуал/CSS не тронуты).
- `ui/app.js`: ячейки → `role="option"` + существующий непустой `aria-label`.
- Проба: ищет `list box` с именем «Эмодзи», внутри него — именованные ячейки
  (`option/list item/table cell/push button`), требуется ≥10 (`MIN_NAMED_CELLS`).
  Глобальный счётчик `list item` заменён на счётчик **внутри активного listbox** с именами
  (прежний порог 100 был глобальным и мог набираться чужими строками).
- `tests/unit/gui-spike-dom.test.mjs`: новый кейс — статическая роль контейнера (`listbox` +
  непустой `aria-label`) и у каждой `.emoji-cell` роль `option` + непустой `aria-label`.

### P2-1. Нет тихого skip на дисплее

- `tests/smoke/gui-window.smoke.test.mjs`: `status === 2` → **fail** (не skip); отсутствие
  скрипта пробы → **fail**; skip только без `DISPLAY/WAYLAND_DISPLAY` или при явном
  `GUI_SPIKE_E2E=0` / `GUI_WINDOW_PROBE_OPTIONAL=1`.
- `tests/smoke/gui-spike.smoke.test.mjs`: тот же opt-out `GUI_WINDOW_PROBE_OPTIONAL=1`;
  отсутствие бинаря на дисплее — fail (было), skip — только без дисплея.
- Добавлены проверки отчёта пробы: `report.pid > 0`, `window.pid == report.pid`,
  `emoji_cells >= 10`.

### P2-3. Живость окна

- `rust/gui-spike/tests/launch.rs`: замер `lived` (spawn → exit); при
  `SPIKE_EXIT_AFTER_MS=9000` требуется `lived >= 8s`, иначе fail с явным сообщением
  «преждевременный exit 0». `SPIKE_BIN_OVERRIDE` — только для мутационной проверки.
- `tests/smoke/gui-spike.smoke.test.mjs`: `livedMs >= SPIKE_EXIT_AFTER_MS - 1000` после
  `exit 0`; `GUI_SPIKE_BIN` — только для мутационной проверки.
- Проба: смерть процесса на любом шаге → `EnvFail` (exit 2) с шагом и кодом, а не «клик не
  сработал» (exit 1). Проверено внешним `kill -9` дочернего процесса.

## Red-логи (до реализации, все сохранены)

| Команда | Результат |
|---|---|
| `node --test tests/unit/gui-spike-dom.test.mjs` | **exit 1**: `#emoji-grid … actual: role="list"` (`red-p1-2-dom.log`) |
| `python3 scripts/gui-window-probe.py …` (UI ещё `role=list`) | **exit 1**: `AT-SPI: нет list box «Эмодзи»` (`red-p1-2-probe.log`) |
| проба + чужой спайк с `GTK_MODULES=gail:atk-bridge` | **exit 1**: `AT-SPI PID 210208 != запущенного 210437` — проба брала первое «discord»-приложение, а не своё (`red-p1-1-foreign-atspi.log`; первая попытка без atk-bridge не воспроизвела: `red-p1-1-foreign.log`, exit 0) |
| `PATH=<stub python3 exit 2> GUI_SPIKE_E2E=1 node --test tests/smoke/gui-window.smoke.test.mjs` | **exit 0**, `skipped 1` — старая политика молча выпадала в skip (`red-p2-1-stub-status2.log`) |
| `GUI_SPIKE_E2E=1 SPIKE_BIN_OVERRIDE=<fake exit 0 через 1s> cargo test … --test launch` | **exit 101**: `окно прожило 1.05s < 8s … преждевременный exit 0` (`red-p2-3-launch.log`) |
| `GUI_SPIKE_E2E=1 GUI_SPIKE_BIN=<тот же fake> node --test tests/smoke/gui-spike.smoke.test.mjs` | **exit 1**: `окно прожило 1004 мс < 9000 мс` (`red-p2-3-smoke.log`) |
| `… GUI_SPIKE_BIN=/tmp/opencode/no-such-spike node --test …` | **exit 1**: `нет бинарника … smoke обязателен` (`p2-1-detect-nobinary.log`) |
| `PATH=<stub python3 exit 2> … node --test tests/smoke/gui-window.smoke.test.mjs` (после фикса) | **exit 1** — статус 2 больше не skip (`p2-1-detect-status2.log`) |
| внешний `kill -9` дочернего спайка во время пробы | проба **exit 2**: `спайк завершился на шаге «AT-SPI» (exit=-9)` (`p2-3-probe-death.log`) |

## Green-логи (после реализации)

| Команда | Результат |
|---|---|
| `node --test tests/unit/gui-spike-dom.test.mjs` | **exit 0**, 21/21 (`green-unit-dom.log`) |
| `cargo build --offline --manifest-path rust/gui-spike/Cargo.toml` | **exit 0** (`green-build.log`) |
| `python3 scripts/gui-window-probe.py --json /tmp/opencode/gui-probe.json --timeout 30` | **exit 0**, `pid=222877`, `window.pid=222877`, cells 120, PNG 1280×820, 3751 цвет (`green-probe.log`, `green-probe.json`) |
| проба при живом **чужом** AT-SPI-экземпляре (pid 219607) | **exit 0**, своё окно `pid=219815` (`green-p1-1-foreign.log`); `xprop`: `_NET_WM_PID=…` (`green-p1-1-xprop.log`) |
| `GUI_SPIKE_E2E=1 cargo test … --test launch -- --nocapture` | **exit 0**, 1 passed, 9.16 s (`green-launch.log`, `green-p2-3-launch.log`) |
| `node --test --test-concurrency=1 tests/smoke/gui-spike.smoke.test.mjs tests/smoke/gui-window.smoke.test.mjs` | **exit 0**, 2/2, skipped 0 (`green-smokes-final.log`) |
| `npm run gate:worker` | **exit 0**, вердикт `passed`: typecheck/ui-tsc/meta 29/unit 261/spike-rust/smoke 22 (skip 0) (`gate-worker-final.log`) |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check` | exit 0 / exit 0 (`clippy.log`) |

## Oracle

- AT-SPI: реальное дерево WebKitGTK — роль `list box` и 120 именованных `option`-ячеек;
  PID приложения из `get_process_id()`.
- X11: `_NET_WM_PID` окна (`xprop`), уникальный заголовок спайка.
- `UI_SNAPSHOT` из stdout реального бинаря (4/100000/24229/10/38).
- DOM: настоящий `ui/app.js` в шиме + статическая разметка `ui/index.html` (роль контейнера).
- Живость: wall-clock от spawn до exit против `SPIKE_EXIT_AFTER_MS`.

## Генерация / мутации

- Генерация: не применимо.
- Мутация «ранний exit 0 с валидными маркерами» (`fake-early-spike.sh`): **KILLED** новыми
  проверками живости (launch + smoke).
- Мутация «чужое AT-SPI-приложение первым»: **KILLED** PID-привязкой (red → green с живым чужим
  экземпляром).
- Мутация «среда недоступна (status=2)»: **KILLED** политикой P2-1.

## Baseline / restore

- Baseline: все прогоны до правок — probe exit 0 (`/tmp/opencode/gui-probe-baseline.json`),
  unit 20/20, launch/smoke зелёные.
- Restore: правки только в разрешённых файлах; после финальных правок пересобрано, логи и md5
  пересняты на финальных байтах.

## Стабильность

- Probe: 4 зелёных прогона (обычный ×3, с чужим экземпляром ×1); 1 EnvFail — намеренный `kill -9`.
- Launch: 2 зелёных прогона по 9.16 s; Smokes: 3 зелёных прогона (2/2, skip 0).
- Один транзиентный сбой хоста: после серии параллельных запусков UI не отдал `UI_SNAPSHOT`
  за 15 s (a11y-мост WebKit), повтор сразу зелёный; теперь такое ловится как `EnvFail`, а не
  маскируется.

## Файлы + md5 (до → после)

| Файл | md5 до | md5 после |
|---|---|---|
| `scripts/gui-window-probe.py` | `0598fee666a76cd14a5effd90d9d260c` | `ef633a2e43a213927e7cd3a5f9db54f4` |
| `tests/smoke/gui-spike.smoke.test.mjs` | `734d58d0593cd8110e94d1b1932496fe` | `7321007b3b414648997a646409167fd4` |
| `tests/smoke/gui-window.smoke.test.mjs` | `9d663736b71acb9ba05023784efd5cf4` | `3c41c820626e2260ef7ea69df3fb9018` |
| `rust/gui-spike/tests/launch.rs` | `402fb1178f2942d54b1a905bb2f89966` | `9cad6d87bfcfa4936367d24b91c36c94` |
| `rust/gui-spike/ui/index.html` | `6ab1434b55edb03824769d3a561c1957` | `295b63b2fa5add22d4c6b787bfad1f28` |
| `rust/gui-spike/ui/app.js` | `579570f937e09452015e5f5da4fca72c` | `c056c93af87d65b7534ad373f51e5b63` |
| `tests/unit/gui-spike-dom.test.mjs` | `b0040aa720c6649cb6f7a23256ebd6ed` | `9a12bb77880f7f70f0781c9f31874e5f` |

Логи и репро-скрипты: `waves/RW-38/workers/W38f/` (`red-*.log`, `green-*.log`,
`p2-1-detect-*.log`, `p2-3-probe-death.log`, `gate-worker-final.log`, `md5-final.txt`,
`fake-early-spike.sh`, `fake-python3-status2.sh`, `green-probe.json`).

## Риски / вопросы

- **Хост-флейк остаётся:** окно иногда умирает за 1,6–3 s. Теперь это красный статус (как и
  требует P2-3), а не зелёный; при повторе на нестабильном хосте возможен честный fail.
- Проба ждёт `UI_SNAPSHOT` до 20 s: при зависшем a11y-мосте это `EnvFail`, а не pass.
- X11-привязка требует `_NET_WM_PID`; если WM не выставляет свойство — `EnvFail`
  (для GDK_BACKEND=x11 это корректно, но Wayland-only хосты пробу не пройдут).
- Порог ячеек в пробе — `≥10` (по ТЗ), при этом проверка стала точнее: только именованные
  ячейки внутри своего `listbox` (прежний глобальный порог 100 мог набираться чужими строками).
- Тест-хуки `SPIKE_BIN_OVERRIDE` / `GUI_SPIKE_BIN` — только env, по умолчанию не заданы;
  гейт их не выставляет. Использованы как мутационные входы (P2-3).
- Вне scope: P2-2, P2-4…P2-10 (роли/имена/заголовки в AT-SPI, `cargo test` в gate,
  Xlib-утечки, timeout/лог, мутации на финальном `app.js`, `--bench-log`, `aria-live`,
  сироты smoke) — не менялись.

## Следующий шаг

Передать REV-2 на повторное ревью RW-38: проверить P1-1/P1-2/P2-1/P2-3, red-логи и границы
файлов; затем приёмка Team Lead и запись в `waves/db.jsonl`/`waves/STATE.md`.
