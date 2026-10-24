# RW-38 — независимое ревью (REV-1)

Дата: 2026-10-23. Ревьюер: REV-1 (не автор кода). Код не менялся: только чтение и прогоны.
Предмет: усиление тестов GUI (DOM-шим, Rust/IPC, AT-SPI/X11, gate), без новых продуктовых фич.

## 0. Что реально прогнано (не только прочитано)

| Проверка | Результат |
|---|---|
| `node --test tests/unit/gui-spike-dom.test.mjs` ×5 | 20/20 pass в каждом прогоне, ~0,1 с |
| `node --test tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-dom.test.mjs` | 91/91 (26 + 45 + 20) |
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | 20/20 ipc_contract + launch «ok» (фактически skip: `GUI_SPIKE_E2E` не задан) |
| `GUI_SPIKE_E2E=1 cargo test … --test launch -- --nocapture` | 1/1, live-окно, 3,4 с |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check` | 0 / OK |
| `npm run typecheck`; `npx tsc -p rust/gui-spike/ui/tsconfig.json` | 0 / 0 (UI-tsc в гейт не входит) |
| `python3 scripts/gui-window-probe.py --json … --timeout 30` | exit 0: 4/10/38, list item 22/24, PNG 1280×820, 3755 цветов |
| `node --test --test-concurrency=1 tests/smoke/gui-spike.smoke.test.mjs tests/smoke/gui-window.smoke.test.mjs` | 2/2 live, 14,7 с, процессов `gui-spi` после прогона не осталось |
| `.gate/worker-2026-10-23T15-26-34-287Z.json` | verdict `passed`, smoke 22/22, skipped 0, warnings 0 |

Окружение этой машины: `DISPLAY=:0` + `WAYLAND_DISPLAY=wayland-1`. При ручных запусках бинаря
наблюдал **нестабильное время жизни окна**: процесс периодически завершается с кодом 0 через
1,6–3 с после `UI_SNAPSHOT` (в логе нет `Gdk-Message`/GBM/panic, `_NET_CLIENT_LIST` пуст).
Отдельный WebKitGTK-тест в те же минуты падал с `Gdk-Message: Error 71 … Wayland display`.
Это свойство хоста, но оно вскрывает пробел в проверках (см. P2-3), а не оправдывает их.

## 1. Находки

### P0

Нет.

### P1

#### P1-1. AT-SPI/X11-проба не привязывает окно и приложение к своему процессу → ложный pass на чужом/старом окне

- `scripts/gui-window-probe.py:94-100` — `atspi_app()` берёт **первое** приложение, в имени
  которого есть «discord» (второй экземпляр спайка; при включённой accessibility — и клиент
  Discord с именем «Discord»).
- `scripts/gui-window-probe.py:163-205` — `x11_capture()` берёт **первое** окно с «discord» в
  заголовке и размером ≥200×200; PID окна (`_NET_WM_PID`) не проверяется.
- `scripts/gui-window-probe.py:82-83` — свой процесс запускается, но его PID нигде не сверяется
  с найденным окном/приложением; в JSON PID тоже не пишется.

Воспроизведено на этой машине (не теория):

```bash
SPIKE_EXIT_AFTER_MS=90000 ./rust/gui-spike/target/debug/discord-gui-spike &   # «старый» экземпляр
# pid 182189, его окно 0x1800003, _NET_WM_PID = 182189
python3 scripts/gui-window-probe.py --json /tmp/opencode/rev1-probe2.json --timeout 30
# -> exit 0, window.xid = 0x1800003, т.е. окно ЧУЖОГО процесса; свой экземпляр пробы жив
xprop -root _NET_CLIENT_LIST   # после выхода пробы остался только 0x1800003 (pid 182189)
```

Следствие: если свежая сборка рендерит пустое/битое окно (но фронт успел вызвать
`report_ui_snapshot`, а Rust его принял), проба снимет PNG и обойдёт AT-SPI **старого здорового**
окна и вернёт `ok: true`. Это ровно «зелёный тест при сломанном продукте».

Тот же сценарий возникает без «сироты»: `npm test` / `npm run test:smoke` запускают файлы
параллельно, `gui-spike.smoke` и `gui-window.smoke` поднимают два окна одновременно —
проба может выбрать окно соседнего теста. `--test-concurrency=1` в gate это маскирует, но сам
инструмент остаётся небезопасным.

**Исправление:** сверять `proc.pid`:
- X11: читать `_NET_WM_PID` (`XGetWindowProperty`) и брать только окно с `pid == proc.pid`;
  если такого нет — `EnvFail` («своё окно не появилось»), если было и исчезло — `Fail`;
- AT-SPI: `app.get_process_id() == proc.pid`; имена приложений/окон — только как вторичный фильтр;
- писать `pid` в JSON и в `--json`-отчёт; при нескольких совпадениях — fail, а не «первое».

#### P1-2. `role="grid"` без строк: эмодзи-сетка не попадает в AT-SPI (пустая таблица), тесты этого не видят

- `rust/gui-spike/ui/index.html:39` — `<div id="emoji-grid" … role="grid">`.
- `rust/gui-spike/ui/app.js:502-508` — ячейки получают `role="gridcell"`/`aria-label`, но
  родительского `role="row"` нет (плоский список ячеек внутри grid).

Факт с живой сессии (дерево AT-SPI сразу после клика по «Эмодзи»):

```
table :: Эмодзи            children = 0
page tab :: Смайлы … Прочее (10, SELECTED выставляется)
entry :: Поиск эмодзи
```

120 ячеек есть в DOM (DOM-тест это подтверждает), но в accessibility-дереве **нет ни одной**:
WebKit отбрасывает `gridcell` вне `row`, и скринридер видит пустую таблицу. Без ролей ячейки —
обычные div с текстом; текущая разметка делает сетку недоступной, а проба проверяет только
10 вкладок и наличие `entry` — поэтому зелёная (`emoji_categories=10`, `ok: true`).
`rust/gui-spike/` в git не отслеживается (каталог целиком untracked), поэтому «было до волны /
добавлено сейчас» по истории не проверить; в любом случае новый AT-SPI-слой это не ловит.

**Исправление (любой из вариантов, без изменения визуала):**
- `role="listbox"` на `#emoji-grid` + `role="option"` на ячейках (плоская структура, валидно), или
- обернуть ячейки в `role="row"` c `display: contents` (CSS-grid не поедет), или
- убрать grid/gridcell и дать ячейке `role="button"` + `aria-label`.
Добавить в пробу: `cells = find_all(app, role in ("table cell","option","button"))`,
`len(cells) >= 10` и непустые имена; в DOM-слой — инвариант «у каждой ячейки роль из
допустимого набора и непустой `aria-label`».

### P2

#### P2-1. На машине с дисплеем live-слой может тихо выпасть в skip, а гейт остаться зелёным

- `tests/smoke/gui-spike.smoke.test.mjs:27` — нет бинаря → `t.skip(...)` **даже при DISPLAY**.
- `tests/smoke/gui-window.smoke.test.mjs:30-34` — `status === 2` (нет AT-SPI/Pillow/бинаря) →
  `t.skip(...)` независимо от наличия дисплея.
- `waves/RW-38/REPORT.md:39` утверждает «skip только без дисплея» — код шире.

Гейт фиксирует skip только warning'ом (`passed_with_skips`), т.е. обязательный по TZ §2.5
live-слой может отсутствовать без красного вердикта. **Исправление:** при `DISPLAY`/`WAYLAND`
и отсутствии `GUI_SPIKE_E2E=0`: нет бинаря/устарел → fail; `status===2` → fail, кроме явного
opt-out (`GUI_SPIKE_E2E=0` или `GUI_WINDOW_PROBE_OPTIONAL=1`); skip — только без дисплея.

#### P2-2. AT-SPI-проба не проверяет роли/имена, а счётчики может набрать с чужого экрана

- `scripts/gui-window-probe.py:258-261` — рейл матчится по `name in (emoji, label)`; при потере
  `aria-label` имя станет «💬», и проба продолжит проходить (доступное имя рейла не проверяется).
- `:273` — вкладки ищутся по имени, роль `page tab` не проверяется; клика по категории нет,
  сетка после клика не проверяется.
- `:305-314` — «list item» считается по всему приложению; если скрытый экран оставит узлы,
  порог ≥10 наберется строками чата, а не участников. Имена строк (`member_\d+_`, `автор: #N`)
  не проверяются.
- `:330-335` — заголовок активного экрана (`.screen-title`, role=heading) не проверяется,
  т.е. «после клика меняется активный экран» доказывается косвенно.

**Исправление:** искать в пределах активного `landmark`/region; требовать роль (`button`,
`page tab`, `table cell`/`option`, `check box`) и ожидаемое имя; после клика по вкладке
сравнить число/первый элемент сетки; после каждого rail-клика сверять heading с
`titles[screen]`; для строк — regex имени.

#### P2-3. `window-ready` + `UI_SNAPSHOT` не доказывают, что окно живо/видимо; преждевременный exit 0 проходит

- `rust/gui-spike/tests/launch.rs:110-135` — проверяются exit 0, маркер и snapshot; время жизни
  окна/видимость не проверяются, хотя тест держит `SPIKE_EXIT_AFTER_MS=9000` и таймаут 20 с.
- `tests/smoke/gui-spike.smoke.test.mjs:51-56` — то же.
- `scripts/gui-window-probe.py:251-314` — между шагами не проверяется `proc.poll()`; если окно
  закрылось (WM/композитор/сбой WebKit), проба падает как «клик не сработал» (exit 1, ложный
  fail), а не как env.

На этом хосте бинарь неоднократно завершался с кодом 0 через 1,6–3 с после snapshot, и оба
smoke/launch оставались зелёными (см. §0). **Исправление:** в launch/smoke после snapshot
дождаться `SPIKE_EXIT_AFTER_MS - 1s` и требовать, чтобы процесс был жив (или чтобы окно было
в `_NET_CLIENT_LIST`); преждевременный exit 0 — fail. В пробе — `proc.poll()` перед каждым
шагом: умер → `EnvFail` с явной причиной.

#### P2-4. Rust-слой не входит ни в один гейт; skip launch выглядит как pass

- `scripts/gate.mjs:113-118` — фазы: typecheck/meta/unit/smoke; `cargo test` для
  `rust/gui-spike` нет (у спайка свой `[workspace]`, корневой `cargo test --workspace` его не
  берёт).
- `rust/gui-spike/tests/launch.rs:83-90` — без `GUI_SPIKE_E2E` тест `ok` (eprintln + return),
  т.е. `cargo test` зелёный, даже если launch сломан.

**Исправление:** добавить в worker/release headless-фазу
`cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --lib --test ipc_contract`;
live launch оставить opt-in, но в отчёте гейта фиксировать, что он не выполнялся (сейчас это
только текст в stderr).

#### P2-5. ctypes: колбэк Xlib-error-handler не удерживается; XImage/Display не освобождаются

- `scripts/gui-window-probe.py:156-157` — `x.XSetErrorHandler(ERR(lambda …))`: временный
  `CFUNCTYPE` после вызова не имеет Python-ссылки; при X-ошибке (BadWindow/BadDrawable —
  например, окно закрылось между `XQueryTree` и `XGetImage`) Xlib вызовет освобождённую
  функцию → UB/segfault пробы (код 139, выглядит как падение продукта).
- `:189-206` — `XGetImage` не `XDestroyImage`, `XOpenDisplay` не `XCloseDisplay`,
  `XFetchName` не `XFree(name)` (утечки на каждый обход).
- `XSetErrorHandler` вызывается без `argtypes/restype`; его возвращаемое значение (указатель)
  обрезается до `int` и записывается обратно в `x.XSetErrorHandler`.

**Исправление:** держать ссылку (`HANDLER = ERR(...)` на уровне модуля / атрибут),
задать `argtypes=[c_void_p]`, `restype=c_void_p`; после использования —
`XDestroyImage(xi)`, `XFree(name)`, `XCloseDisplay(dpy)` в `finally`.

#### P2-6. `--timeout` управляет временем жизни приложения, а не только пробой; фиксированный LOG; утечка child

- `scripts/gui-window-probe.py:251` — `SPIKE_EXIT_AFTER_MS = (timeout-5)*1000`; при
  `--timeout 10` приложение самоубивается через 8 с, а проверок ~7–8 с → плавающий fail.
  Валидации «timeout ≥ времени проверок» нет (`:243` проверяет только ≥5).
- `:58-91` — при таймауте `window-ready`/исключении после `Popen` процесс не убивается
  (`proc` в `main` ещё `None`), файл лога не закрывается.
- `:30` — `LOG` фиксирован (`/tmp/opencode/gui-window-probe.log`), `:81` открывает его на
  запись: два параллельных прогона (или проба + `run-mutations-gui.py`) затирают лог друг друга,
  и `analyze_log` может прочитать чужой/усечённый snapshot.

**Исправление:** lifetime = `max(30, timeout)` и отдельный параметр; `try/finally` вокруг
`Popen` с `terminate/kill`; уникальный путь лога (`tempfile.mkdtemp` / pid в имени) и передавать
его в JSON.

#### P2-7. Мутационное доказательство относится к другому `app.js`, чем сдаваемый; AT-SPI-мутант один

- `waves/RW-38/workers/W38a/REPORT.md:82,103` — мутации/зелёный baseline сняты на
  `app.js md5 32ed027b…`; сдаваемый файл — `c882792f…` (ARIA-правки W38c уже после). Логи
  `mut-M*.log` не содержат md5 входа. На финальном `app.js` тесты зелёные (проверил 20/20 ×5),
  но red-first по M1–M5 к финальному файлу не перепрогнан.
- `waves/RW-38/mutations-gui.log` — G1 (`MEMBER_ROWS→100`) пойман **валидатором Rust**
  («UI_SNAPSHOT отсутствует»), а не AT-SPI; реально AT-SPI-инвариант проверяет только G2
  (38→37). Роли/имена/клики/PNG мутациями не подтверждены.
- `waves/RW-38/run-mutations-gui.py:22` — мутирует **сдаваемый** `ui/app.js` в рабочем дереве
  (в отличие от копий у W38a); `finally` восстанавливает, но SIGKILL/обрыв оставит файл
  мутированным.

**Исправление:** перепрогнать `run-mutations.sh` на финальном `app.js` и писать md5 в шапку
каждого лога; для live — собирать копию спайка в `/tmp` (или хотя бы `trap` на SIGINT/SIGTERM
с restore+rebuild); добавить мутанты: убрать `role="listitem"`/`aria-label` у строк, убрать
`aria-label` у рейла (ожидаемо должен ловиться после P2-2), сделать `switchScreen` no-op,
снять `role="tab"`, сломать структуру сетки (P1-2), снять `checked` у тумблера.

#### P2-8. `--bench-log` без строки `BENCH` → exit 0 и `bench: null`

- `scripts/gui-spike-bench.mjs:435-438` — печатает ошибку в stderr, но возвращает `EXIT_OK`;
  `tests/unit/gui-spike-bench.test.mjs:497-513` закрепляет это как контракт.
- Следствие: `metrics.json` может не содержать `fps`/`p95`, хотя DoD §2.5 требует их; вызывающий
  скрипт (`remeasure.py`) проверяет `BENCH` отдельно, но ручной прогон по RUNBOOK §3 — нет.

**Исправление:** при явно заданном `--bench-log` и отсутствии `BENCH` возвращать `EXIT_ENV` (2)
либо писать `bench_missing: true` и требовать это поле в приёмке; обновить тест.

#### P2-9. `aria-live="polite"` на блоке метрик + запись FPS каждый кадр — шум для скринридера и нагрузка на a11y-мост

- `rust/gui-spike/ui/index.html:21`; `rust/gui-spike/ui/app.js:205` (`el("m-fps").textContent`
  в `frameLoop`, каждый rAF).

Живой регион меняется на каждом кадре; при включённом AT-SPI это дополнительный поток
изменений текста в live-region (шум для скринридера и лишняя работа моста), хотя визуал не
страдает — заявленное «без изменения производительности» и доступность под вопросом.
(Замер a11y-событий не проводился — это риск, а не измеренный факт.)

**Исправление:** убрать `aria-live` с `.metrics` (или оставить только для RAM/потоков с
`aria-live="off"` у FPS/p95), обновлять FPS не чаще 1 Гц и только при изменении значения.

#### P2-10. Smoke не убивает child при таймауте теста → сиротское окно (питает P1-1)

- `tests/smoke/gui-spike.smoke.test.mjs:34-51` — `spawn` без `t.after`/`finally`; при
  `{ timeout: 20_000 }` тест падает, а процесс с окном живёт до своего `SPIKE_EXIT_AFTER_MS`
  (или дольше, если env не сработал) — следующая проба может поймать это окно.
- `tests/smoke/gui-window.smoke.test.mjs:26-29` — `spawnSync(timeout: 80_000)` убивает только
  python; дочерний спайк остаётся.

**Исправление:** `t.after(() => child.kill("SIGKILL"))` + `AbortSignal.timeout`; в
`gui-window.smoke` — `spawn` с ручным kill по таймауту либо передать в пробу обработчик
SIGTERM, который гасит child (сейчас `finally` в пробе не выполняется при SIGKILL).

### P3

1. `tests/support/gui-dom-harness.mjs:280-284` — `appendChild` не переносит узел из старого
   родителя (в DOM переносит). Сейчас app.js так не делает, но будущая ошибка «строку
   добавили второму родителю» шимом не ловится (в старом останется фантом). Добавить
   отсоединение от `child.parentNode` и защиту от циклов.
2. `tests/support/gui-dom-harness.mjs:232-243` — `textContent` при наличии детей игнорирует
   собственный `nodeText` (в DOM конкатенирует). `:337-353` — нестандартная сигнатура
   `dispatchEvent(type, init)` вместо `dispatchEvent(event)`: будущий тест с `new Event()` тихо
   не сработает. `:980-990` — часы двигаются на 16,7 мс **на каждый колбэк**, а не на кадр
   (исказит bench, если его начнут гонять в шиме). `:1019-1027` — `dispose()` не снимает
   глобалы и не отписывает слушатели элементов.
3. `tests/support/gui-dom-harness.mjs:561` — неизвестный синтаксис селектора (`>`, `+`, `~`,
   `:not`, `[attr^=]`) молча даёт пустой список (комментарий это признаёт). Лучше бросать
   `Error("неподдерживаемый селектор")`, чтобы тест падал явно, а не «на пустоте».
4. `tests/unit/gui-spike-dom.test.mjs:319-333` — `loadedRows ≤ 500` проверяется только по
   snapshot, снятому при загрузке; после свипа 0/50k/100k бюджет не перепроверяется.
   Регрессия `CHAT_MAX_PAGES=50` (5000 строк в heap) пройдёт DOM-тесты (модельные тесты
   используют свои `maxPages`). Добавить post-scroll проверку (второй snapshot/`stats()`) или
   мета-тест на константы `PAGE_SIZE/MAX_PAGES` из app.js.
5. `rust/gui-spike/tests/launch.rs:66-70` — `WAYLAND_DISPLAY` считается достаточным, но
   `GDK_BACKEND=x11` форсируется: на чистом Wayland без XWayland это fail, а не skip.
   `:51-56` — `join()` читателей без таймаута: если WebKit-потомок удержит пайп после exit
   родителя, тест повиснет (20-секундный дедлайн покрывает только `try_wait`). Нет проверки
   свежести бинаря относительно `ui/*` (в Node-smoke она есть).
6. `scripts/gui-window-probe.py:103-114` — `find_all` глотает исключения и ограничен depth 14:
   частичное дерево может дать как ложный fail, так и «достаточный» счётчик. `:182` — приоритет
   окна по подстроке «GUI spike»; `XFetchName` декодируется как UTF-8, в JSON заголовок выглядит
   как `discord-lab � GUI spike` (COMPOUND_TEXT). `:204` — порог `unique_colors > 1000`
   произволен (для окна 1280×820 это ~0,1% пикселей; нормально, но стоит зафиксировать в
   комментарии/константе).
7. `scripts/gui-spike-bench.mjs:316-333` — `parseBenchLog` принимает любой JSON-объект
   (`BENCH {}` → `bench: {}`), схема `chat/members` не проверяется. Добавить проверку
   `chat/members` (как в Rust `validate_bench_payload`).
8. `waves/RW-38/METRICS.md:3` — хеш бинаря `c7240b…`, фактический `target/debug/discord-gui-spike`
   после restore-пересборки — `715837…`; метрики сняты не с текущего артефакта. Либо
   перезамерить последним шагом и записать хеш, либо явно указать «бинарь до restore-пересборки».
9. `waves/RW-38/TZ.md:42` — AT-SPI-лейн закреплён за W38c, но `workers/W38c/` нет; PM взял
   его на себя (`REPORT.md:33`), red-лог/шаг «до» для пробы не сохранён (есть только мутации
   после). Для приёмки нужен хотя бы один red-прогон пробы (например, на сломанном
   `aria-label`/клике) и фиксация md5.
10. `rust/gui-spike/src/main.rs:31-43` — обёртки `#[tauri::command] report_bench/report_ui_snapshot`
    (валидация+печать) не покрыты ни одним тестом: ipc_contract тестирует только lib, launch
    идёт с `SPIKE_AUTOBENCH=0` и не печатает `BENCH`. Добавить тест бинаря с `autobench` или
    вынести печать в lib-функцию и покрыть.
11. `rust/gui-spike/ui/app.js:642` — `actions.listScroll` = «scrollTop изменился и строк < 100000»;
    это истинно и при no-op paint. В live-слое скролл ничем не подтверждён (проба не скроллит).
    Сделать проверку по факту окна (индексы строк после `scrollToIndex(500)`).

## 2. Аудит-пункт → закрыт?

| Аудит-пункт (TZ/ревью) | Закрыт? | Комментарий |
|---|---|---|
| DOM-шим без зависимостей, реальный `app.js` | ✅ | 1055 строк, 20/20 ×5; ограничения задокументированы |
| 4 экрана, 100k, 24 229, 10, 38 | ✅ | DOM-тесты + live snapshot/AT-SPI; числа из контракта, не из app.js |
| Виртуализация: sizer, окно, удаление строк, рост DOM | ✅ | sizer/`top`/`parentNode=null`/свип — сильный оракул |
| `PageStore.loadedRows ≤ 500` | ⚠️ | Только в snapshot при загрузке; post-scroll не проверяется (P3-4) |
| Переключение экранов/категорий/тумблеров | ✅ | DOM; live — рейл и один тумблер (P2-2) |
| `UI_SNAPSHOT`: все поля + actions | ✅ | DOM-тест + Rust floors; `listScroll` слабый (P3-11) |
| Rust lib/разделение, поведение не изменилось | ✅ | Тонкие обёртки, тексты ошибок сохранены, API минимален |
| IPC-контракт (Ok/Err, типы, границы) | ✅ | 20/20, включая отрицательные/типы/границы |
| Live launch (окно, snapshot, GBM/panic, skip-aware) | ⚠️ | Работает, но нет проверки жизни/видимости окна (P2-3); skip=pass (P2-4) |
| AT-SPI: окно, элементы, клики, PNG, exit 0/1/2 | ⚠️ | Клики реальные, но нет привязки к своему PID (P1-1), роли/имена не проверяются (P2-2) |
| ARIA/доступность ролей и имён | ❌ | Эмодзи-сетка пуста в AT-SPI (P1-2); `aria-live` FPS (P2-9) |
| Gate: live smoke при дисплее, GBM/panic → fail, stale binary → fail | ⚠️ | Запись гейта 22/22, 0 skip; но skip при отсутствии бинаря/env (P2-1), Rust вне гейта (P2-4) |
| `metrics.json` получает fps/p95 из `BENCH` | ✅/⚠️ | Числа на месте; без `BENCH` — exit 0 и `bench:null` (P2-8) |
| Мутации по новым инвариантам | ⚠️ | M1/M3/M4/M5 — на старом app.js (P2-7); AT-SPI — только G2 |
| Качество: clippy/fmt/tsc/тесты | ✅ | clippy 0, fmt OK, tsc 0 (UI-tsc вручную), 20+20+91+2 live |
| RUNBOOK/воспроизводимость | ⚠️ | Команды рабочие; хеш бинаря в METRICS и отсутствие W38c-отчёта (P3-8/9) |
| Нет ложных pass | ❌ | P1-1; P2-1; P2-3 |

## 3. Минимум до повторной приёмки

1. P1-1: привязка окна/приложения к `proc.pid` (`_NET_WM_PID`, `Atspi.get_process_id`),
   fail вместо «первого совпадения»; уникальный лог; kill child в `finally`.
2. P1-2: валидная структура сетки (`listbox/option` или `row`-обёртки) + проверка ячеек
   в пробе; иначе убрать grid-роли.
3. P2-1/P2-3: на дисплее нет бинаря/`status=2` → fail (кроме явного opt-out); проверять, что
   окно живо на горизонте `SPIKE_EXIT_AFTER_MS`, а проба — `proc.poll()`.
4. P2-2: роли/имена/заголовок/клик по категории в AT-SPI-пробе.
5. P2-4: headless `cargo test` спайка в `gate:worker`/`release`.
6. P2-5: удержать Xlib-error-handler, `XDestroyImage`/`XFree`/`XCloseDisplay`.
7. P2-7: перепрогнать мутации на финальном `app.js`; добавить role/click-мутанты.

## 4. Вердикт

**FAIL** — P0 = 0, **P1 = 2** (P1-1 ложный pass AT-SPI/X11 на чужом окне; P1-2 недоступная
эмодзи-сетка). P2 = 10, P3 = 11. После исправления P1 и P2-1…P2-5 волну можно принимать
повторно; остальное — не блокирует, но должно попасть в follow-up.
