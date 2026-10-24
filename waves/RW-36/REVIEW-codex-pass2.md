# RW-36 / B07 — код-ревью REV-1, второй проход (codex, pass 2)

Дата: 2026-10-23. HEAD: `0239543`. Ревьюер: REV-1 (не автор кода), второй проход по фиксам
после `waves/RW-36/REVIEW-codex.md` (pass 1: P0=0, P1=4, P2=11).

Файлы под ревью (md5 на момент прочтения):

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/main.rs` | `e931bb79607822f12e458be9233e510c` |
| `rust/gui-spike/ui/app.js` | `0d9c02a6fa7ac7a9d01e6bed6d83e1c4` |
| `rust/gui-spike/ui/tsconfig.json` | `d0dd2b8bc3f44f39651ae5fc91071b51` |
| `rust/gui-spike/.gitignore` | `96 байт`, содержимое: `/target/`, `/gen/` |
| `rust/gui-spike/ui/index.html` | `83be4cf5ef6435e4e22447bbaf0c027a` (не менялся) |
| `rust/gui-spike/tauri.conf.json` | `eb99adb3481255a3de282853a0c55f8c` (не менялся) |
| `rust/gui-spike/capabilities/default.json` | `f1cab1e9cf69fc41c55dd1106c41ac74` (не менялся) |
| `scripts/gui-spike-bench.mjs` | `dd3ae8e1ca0389fbde39740cb72a5d15` |
| `tests/unit/gui-spike-bench.test.mjs` | `c32183a08282c3370a09f3ab31fd84ca` |
| `tests/unit/gui-spike-dom.test.mjs` | `d7bcf9f092b5408d3fc2a2d2834bb34b` |
| `tests/smoke/gui-spike.smoke.test.mjs` | `059fa2782a6ac9347a0825fc40ef6293` |
| `waves/RW-36/METRICS.md` | `b2ef1aee11b6803adc49f9915b7db570` |
| `docs/rust-client/ADR-1.md` | `655a34373ef9255ec3a721928484191d` |

## Что реально прогнано

| Проверка | Результат |
|---|---|
| `cargo test --offline` в `rust/gui-spike` | **12/12 pass** (`Compiling … in 0.43s`, все 12 имён из отчёта воркера) |
| `node --test tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-dom.test.mjs` | **37/37 pass** (34 в bench + 3 в dom) |
| `node --test tests/meta/plan-sync.test.mjs` | **5/5 pass** (§8 ↔ тесты ↔ модули консистентны) |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | **clean, exit 0** (DOM-типы, `strict`, `noUncheckedIndexedAccess`) |
| `git check-ignore -v` + `git add -n rust/gui-spike` | `target/`/`gen/` игнорируются; в индекс идут только 12 исходников |
| эмпирика `--tree` (внешний оракул) | parent 66 632 + child 79 252 = **145 884** = tree `rss_kb.avg`; `processes=2`, `hwm=max(…)`, `threads=7+7=14`, `skipped=0` |
| воспроизводимость сборки | два последовательных `cargo build` из финальных исходников дают один и тот же sha256 `d4d6014…` (детерминированно) |

Не запускал: live smoke/GUI-окно (нет согласованного окна), `gate:worker` и `cargo test --workspace`
(правило «один cargo за раз», чужой product-target не трогаю). Побочный эффект ревью: в
`rust/gui-spike/target/debug/` пересобран бинарник (см. P2-2), исходный артефакт замера сохранён
в `/tmp/opencode/spike-bin-1857` (sha256 `f33ee542…`). Код не менялся.

## Итог по приоритетам

| Приоритет | Кол-во |
|---|---|
| P0 | 0 |
| **P1** | **0** |
| P2 | 13 |
| P3 | 20 |

**Вердикт: PASS по критерию «P0/P1 = 0».** Все четыре P1 первого прохода закрыты (доказательства
ниже). Остаются 13 P2: часть — новые (в основном «фикс есть, теста/гейта нет»), часть —
перенесённые из pass 1. По TZ §4.6 приёмка требует, чтобы P2 были закрыты **или** осознанно
отложены с записью в `waves/RW-36/DECISIONS.md` (файла пока нет). Отдельно P2-2 (артефакты сняты
до финальных правок) обязан быть закрыт перезамером **до** фиксации sha в журнале волн.

---

## P0 — нет

Целенаправленно перепроверял: `unsafe`/паники на вводе, запись в ФС, сеть, инъекции в лог и DOM,
path traversal, утечки таймеров/хендлов. `main.rs` читает только `/proc/self/status`; фронт
локальный; весь пользовательский текст — через `textContent`; новых внешних входов нет.
`report_bench`/`report_ui_snapshot` теперь валидируются (см. P2-8 про тесты).

## P1 — нет

---

## Таблица: P1 первого прохода → статус

| P1 pass 1 | Статус | Доказательство |
|---|---|---|
| **P1-1** Виртуализация не перерисовывается при показе/resize | **CLOSED** | `app.js:62-65` — `ResizeObserver(() => this.paint())` на контейнере; `app.js:247-249` — `switchScreen()` после `classList.add("active")` планирует `requestAnimationFrame(() => list.paint())`; `app.js:468-471` — `memberRenderedRows` считается **после** `switchScreen("members")` + `await nextFrame()`. Эмпирика `waves/RW-36/app.log:2`: `"memberRenderedRows":24` (в pass 1 было 8), `"chatRenderedRows":30`; `METRICS.md:49-57`. Проверил и «обратный» сценарий: `paint()` читает фактический `scrollTop`/`clientHeight` заново, поэтому после `display:none → active` диапазон пересчитывается от реального состояния, пустого вьюпорта нет. |
| **P1-2** `benchScroll` без таймаута / тихое незавершение | **CLOSED** (с оговоркой) | `app.js:173-198` — сторож `setTimeout(…, durationMs + 2000)`, выставляет `stalled = true` и резолвит промис; `app.js:200-211` — вырожденная выборка больше не даёт `Infinity`: `deltas.shift()` → `frames` 0, `avg` 0, `fps: avg > 0 ? … : 0`; `app.js:215` — `benchRunning = false`. Строка `BENCH` теперь печатается всегда, в т.ч. при `stalled` (`app.log:3` — валидный прогон). Оговорки: нет порога `frames >= 30` (частично тикающий rAF даст `fps≈1` без пометки) и сброс `benchRunning` не в `finally` → P2-5. |
| **P1-3** RAM мерилась только по родителю | **CLOSED** | `gui-spike-bench.mjs:235-262` (`readTreeStatus`: `/proc/<pid>/task/*/children` по всем потокам, рекурсия стеком, `seen`), `:211-221` (`aggregateTree`: RSS-сумма, HWM-max, threads-сумма), `:366-377` (`processes` = max за замер, `skipped` = сумма); `tests/unit/gui-spike-bench.test.mjs:230-321`; артефакт `metrics-tree.json` (573 МБ, 3 процесса, skipped 0); `METRICS.md:87-100` — PSS-таблица и явная оговорка «RSS-сумма завышена shared-страницами»; `ADR-1.md:36-37` + §4.2 «метрика памяти — PSS дерева». TZ §3/§4 п.3 обновлён под `--tree`/PSS. Эмпирически подтвердил отсутствие двойного счёта (см. таблицу выше). |
| **P1-4** `target/`/`gen/` не в `.gitignore` | **CLOSED** | `rust/gui-spike/.gitignore:2-3` (`/target/`, `/gen/`); `git check-ignore` подтверждает оба пути; `git add -n rust/gui-spike` перечисляет только `.gitignore`, `Cargo.lock/toml`, `build.rs`, `capabilities/`, `icons/`, `src/`, `tauri.conf.json`, `ui/*` — 12 файлов, ни `target/`, ни `gen/`. `Cargo.lock` не игнорируется (правильно). |

Все четыре — закрыты по коду. Остаточные риски (не блокируют): слабая нижняя граница rendered-строк
в smoke (P2-1), отсутствие PSS-артефакта (P2-6), `unwrap_or(0)` (P2-7).

---

## P2 — средние

### P2-1. Smoke-тест по-прежнему пропускает регрессию P1-1: `memberRenderedRows > 0`
`tests/smoke/gui-spike.smoke.test.mjs:45`, `rust/gui-spike/src/main.rs:68-74`

Фикс P1-1 в коде есть, но единственный автоматический страж DoD P0-06 всё ещё принимает 8 строк
(именно столько было при дефекте): `assert.ok(snapshot.memberRenderedRows > 0 && …)`. Валидатор
бэкенда тоже не проверяет rendered-строки: `SNAPSHOT_FLOORS` содержит только `chatRows`/
`memberRows`/…, поэтому снимок `{"memberRenderedRows":8,…}` пройдёт и `report_ui_snapshot`.
**Исправление:** floor в валидаторе и smoke — `memberRenderedRows >= 15` (для окна 1280×820
фактически 24; 15 — с запасом на minHeight 600), `chatRenderedRows >= 15`; в smoke
`assert.ok(snapshot.memberRenderedRows >= 15)`.

### P2-2. Артефакты приёмки сняты до финальных правок; sha256 в METRICS невоспроизводим
`waves/RW-36/app.log` (mtime 18:57:19), `rust/gui-spike/src/main.rs` (18:58:33),
`rust/gui-spike/ui/app.js` (18:59:33), `waves/RW-36/METRICS.md:20`

`app.log` (UI_SNAPSHOT/BENCH) и `metrics-tree.json` записаны в 18:57, а `main.rs` и `app.js`
изменены позже — т.е. замер сделан не с финальных байтов. Дополнительно: `cargo build` из
финальных исходников даёт **sha256 `d4d601420e39ea7060a60f11a63fbae66a88f5a2a39645165cbd949367696a2f`**,
а в `METRICS.md:20` зафиксирован `f33ee542…` (файл `target/debug/discord-gui-spike` на 18:57 —
совпадал с `f33ee54…`). Сборка детерминирована (два rebuild подряд — один хеш), значит
`f33ee54…` из текущего дерева не воспроизводится. `metrics.json` вообще от 17:25 (pid 47356),
т.е. RAM-числа сняты с ещё более раннего прогона, а `metrics-tree.json` — с 18:57 (pid 82699);
в `METRICS.md` это нигде не сказано.
Что это **не** ломает: `app.log:2` с `memberRenderedRows=24` сам доказывает, что repaint при
показе экрана в измеренном бинаре уже был (8 строк при `clientHeight=0` получить 24 нельзя), а
правки после замера — валидация `main.rs` (`SNAPSHOT_FLOORS`, `report_bench`) и типизация/замер
`app.js`, которые на FPS/RSS/число строк не влияют по смыслу. Но формально ни один артефакт не
подтверждает именно финальные байты, а `f33ee54…` — не тот хеш, который получит любой, кто
пересоберёт дерево.
**Исправление:** пересобрать, перезапустить живой прогон (нужно окно, согласованное
координатором), переснять `app.log`, `metrics.json`, `metrics-tree.json`, `pss.log`, sha256 в
`METRICS.md:20` и указать pid/время каждого артефакта; без этого `METRICS`/`ADR-1` нельзя
замораживать в журнале волн.

### P2-3. `nextFrame()` без таймаута — тот же класс, что закрытый P1-2
`rust/gui-spike/ui/app.js:413, 462, 470`

`const nextFrame = () => new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));`
— сторож добавлен только в `benchScroll`. На occluded/software-рендере (ровно тот сценарий, из-за
которого появился guard) rAF может не тикать: `reportUiSnapshot()` навсегда останется на
`await nextFrame()`, `UI_SNAPSHOT` не напечатается, а в `app.log` не будет ни строки, ни причины.
Отличие от P1-2: отсутствие снимка обнаруживается smoke-тестом (`assert.ok(line)`), т.е. это не
тихое неверное число, поэтому P2, а не P1.
**Исправление:** `const nextFrame = (ms = 2000) => Promise.race([new Promise(r => requestAnimationFrame(r)), new Promise((_, rej) => setTimeout(() => rej(new Error("rAF не тикал")), ms))])`
и обернуть `reportUiSnapshot()` в try/catch с явным маркером (см. P2-4).

### P2-4. Отказ валидации не виден в `app.log`
`rust/gui-spike/src/main.rs:53-64, 91-96`, `rust/gui-spike/ui/app.js:434-438, 483-485`

`report_bench`/`report_ui_snapshot` возвращают `Err`, но причина уходит только в `console.error`
webview, а он в `app.log` не попадает (там stdout+stderr процесса, не консоль WebKit). При
провале floor'а или невалидном JSON в логе просто нет `BENCH`/`UI_SNAPSHOT`, и харнесс не отличит
«не снялось» от «отвергнуто». Это то же «не глотать молча».
**Исправление:** перед `Err` печатать `eprintln!("UI_SNAPSHOT_REJECTED {err}")` /
`eprintln!("BENCH_REJECTED {err}")` (stderr уже идёт в `app.log` через `2>&1`).

### P2-5. `benchRunning` не сбрасывается в `finally`; ошибки бенча не ловятся; нет порога `frames`
`rust/gui-spike/ui/app.js:205-215, 219-223, 428-431`

`benchRunning = false` стоит после `await`, а не в `finally`. Любой throw между `benchRunning = true`
и этой строкой (например, `el("m-p95")` вернул `null` → TypeError) навсегда блокирует бенч:
ручная кнопка молча ничего не делает (`if (benchRunning) return null`), а `autoBench` падает в
unhandled rejection без `BENCH`. Обработчик клика (`:222`) и `autoBench` (`:428`) `benchScroll`
не оборачивают. Плюс нет порога валидности выборки: при частично тикающем rAF (5 кадров за 5 с)
получится `frames=4, fps≈0,8` без `stalled`, т.е. мусорное число в `BENCH` без пометки.
**Исправление:** `try { … } finally { benchRunning = false; }` внутри `benchScroll`; `frames < 30`
→ вернуть `{ frames, ..., fps: 0, error: "too-few-frames" }` и всё равно отправить `report_bench`
(чтобы провал был виден в логе); `benchScroll(...).catch(console.error)` в обработчике и `try`
вокруг блока в `autoBench`.

### P2-6. PSS — центральная цифра ADR, но нет артефакта замера
`waves/RW-36/METRICS.md:87-100`, `docs/rust-client/ADR-1.md:36`

`399 042 КиБ ≈ 390 МБ` и таблица по процессам названы «ручной замер»; в `waves/RW-36/` нет ни
`smaps_rollup`-дампа, ни скрипта/лога, из которого эти числа получены. Именно на этой цифре
держится вывод «бюджет ≤300 МБ не выполнен» (§3 ADR). Внутренняя согласованность есть
(81 + 277 + 31 ≈ 389 МБ; RSS-сумма 573 МБ), но проверить нечем.
**Исправление:** сохранить `waves/RW-36/pss.log` (`for p in …; do echo $p; cat /proc/$p/smaps_rollup; done`)
и сослаться на него в `METRICS.md:89`; либо добавить в скрипт режим `--pss`, который суммирует
`Pss:` по дереву (тогда цифра станет воспроизводимой одной командой).

### P2-7. `parse_process_metrics` глушит отсутствие полей через `unwrap_or(0)`
`rust/gui-spike/src/main.rs:28-34`

P2-3 первого прохода не закрыт: `rss_kib: read_status_field(status, "VmRSS:").unwrap_or(0)`.
`0` неотличим от реального значения и от ошибки чтения; UI покажет «0 МБ». Тест на отсутствие
поля (`parse_process_metrics("Name:\tx\n")` → ошибка) не добавлен.
**Исправление:** `read_status_field(...).ok_or_else(|| format!("нет поля {key} в /proc/self/status"))?`
и `parse_process_metrics(status) -> Result<ProcessMetrics, String>`; тест на каждый пропущенный ключ.

### P2-8. Валидация `report_bench` не покрыта ни одним тестом
`rust/gui-spike/src/main.rs:53-64`, `rust/gui-spike/src/main.rs:123-244` (модуль тестов)

Новый код (JSON-парсинг + требование объектов `chat`/`members` + печать `{value}`) не тестируется:
в `mod tests` импортированы только `parse_process_metrics`, `read_status_field`,
`validate_ui_snapshot`. Мутанты «`println!("BENCH {payload}")`» (возврат инъекции), «убрать проверку
`chat`», «не проверять JSON» выживают; в `mutations-rust.log` мутаций на `report_bench` нет.
**Исправление:** `#[test] report_bench_rejects_non_json()`, `report_bench_rejects_missing_chat()`,
`report_bench_rejects_missing_members()` (Err-пути вызываются напрямую, `#[tauri::command]`-функция
вызываема как обычная). Там же добавить лимит длины payload (4 КиБ) — сейчас размер не ограничен.

### P2-9. `ui/tsconfig.json` не подключён к гейту
`package.json:11` (`typecheck`), `rust/gui-spike/ui/tsconfig.json:1-17`

Конфиг создан и **чист** (проверил: exit 0), но `npm run typecheck` = `tsc -p tsconfig.json &&
tsc -p tsconfig.agent.json`; `rust/**` не входит ни в один из них. Т.е. `// @ts-check` в `app.js`
по-прежнему декоративен для гейта: регрессия типов (и `el()`-null) пройдёт `gate:worker`.
**Исправление (TL, общий файл):** `"typecheck": "tsc -p tsconfig.json && tsc -p tsconfig.agent.json && tsc -p rust/gui-spike/ui/tsconfig.json"`.

### P2-10. RUNBOOK не обновлён под новую методику
`waves/RW-36/RUNBOOK.md:41-44, 54-55`

RUNBOOK (mtime 17:02, не менялся) всё ещё велит мерить RAM без `--tree` и тут же сравнивает с
«RAM в простое ≤ 300 МБ» — это ровно та ошибка методики, из-за которой был P1-3: оператор,
идущий по runbook, получит 181 МБ и «зелёный» бюджет. Нет также `SPIKE_AUTOBENCH`, формата
`BENCH`/`UI_SNAPSHOT`, PSS-замера и render-флагов (`LIBGL_ALWAYS_SOFTWARE`,
`WEBKIT_DISABLE_DMABUF_RENDERER`), без которых, по `METRICS.md:40-43`, rAF не тикает.
**Исправление:** переписать разделы «Замер памяти»/«Замер FPS» по `METRICS.md` (файл TL).

### P2-11. Smoke-тест не задаёт `LIBGL_ALWAYS_SOFTWARE=1` — риск флака
`tests/smoke/gui-spike.smoke.test.mjs:18-25`, `waves/RW-36/METRICS.md:40-43`

Smoke выставляет `WEBKIT_DISABLE_COMPOSITING_MODE=1` и `WEBKIT_DISABLE_DMABUF_RENDERER=1`, но не
`LIBGL_ALWAYS_SOFTWARE=1`, хотя `METRICS.md` называет оба флага критичными для того, чтобы rAF
тикал. На этой машине (X11 + программный рендер) smoke рискует упасть на
`assert.ok(line, "UI_SNAPSHOT отсутствует")` — а из-за P2-3 причина будет только «нет строки».
**Исправление:** добавить `LIBGL_ALWAYS_SOFTWARE: process.env.LIBGL_ALWAYS_SOFTWARE ?? "1"` и
`GDK_BACKEND: process.env.GDK_BACKEND ?? "x11"`; в сообщении ассерта печатать `stdout`.

### P2-12. «avg в бюджете» — неверное утверждение (16,67 мс > 16 мс)
`waves/RW-36/METRICS.md:123`, `docs/rust-client/ADR-1.md:30`

Бюджет `ARCHITECTURE.md` §9 — «60 fps, ≤ 16 мс кадр». Факт: avg `16,67 мс`, p95 `18 мс`.
`METRICS.md:123` пишет «avg в бюджете, p95 на 2 мс выше», ADR — «✅/⚠️ p95 +2 мс», т.е. оба
документа занижают расхождение: avg тоже выше 16 мс (на 0,67 мс; это ровно период 60 Гц,
округление `toFixed(2)`). Вывод по тулкиту не меняется, но колонка «Итог» вводит в заблуждение.
**Исправление:** «avg 16,67 мс = период 60 Гц (округление); формально бюджет ≤16 мс превышен на
0,67 мс по avg и на 2 мс по p95; критерий TZ §4 (FPS ≥55, p95 ≤20 мс) выполнен».

### P2-13. Границы `parseCliArgs` не покрыты (P2-9 pass 1 закрыт частично)
`tests/unit/gui-spike-bench.test.mjs:405-424`

Добавлено только 3 кейса (`defaults`, `--tree/--json`, «без --pid»). Нет `--pid 0`,
`--pid 4194305` (2^22+1), дублей `--pid`/`--json`, `--json ""`, «exit 2 → файл `--json` не
создан». Мутанты «убрать `value > max`», «убрать `value < 1`», «разрешить пустой путь» выживают.
**Исправление:** табличные `assert.throws(() => parseCliArgs([...]), /--pid/)` для границ; кейс
`runCli([... , "--json", ""])` → exit 1; кейс «несуществующий pid + `--json`» → exit 2 и
`existsSync(out) === false`.

---

## P3 — мелкие

1. `--tree` не сигнализирует о `skipped > 0`: при недоступном `/proc` потомка сумма тихо
   занижается, exit 0, stderr пуст (`gui-spike-bench.mjs:258, 366-377`). Хотя бы `console.error`
   при `skipped > 0` (в JSON поле есть, но его никто не читает).
2. `readTreeStatus`: если у потомка не прочитался и список детей, и status, `skipped`
   инкрементится один раз вместо двух (`gui-spike-bench.mjs:249-258`) — счётчик занижен.
3. `resolve(argv[1])` вместо `realpathSync` — запуск через симлинк «молча выходит» с exit 0
   (`gui-spike-bench.mjs:392-393`).
4. Ошибка записи `--json` → exit 2 (`gui-spike-bench.mjs:379-386`); по TZ §3 это ошибка
   аргументов (exit 1) либо путь надо валидировать до замера.
5. Дубли флагов молча перезаписываются, `--help` нет (`gui-spike-bench.mjs:315-333`).
6. `el()` кастует `getElementById` без проверки null (`app.js:117`); `strict` это не ловит из-за
   JSDoc-каста, ошибка выйдет как `TypeError: Cannot set properties of null`.
7. `frameLoop`/`pollProcessMetrics` продолжают писать DOM и делать IPC **во время** замера
   (`app.js:127-154`); p95 включает эти спайки. P2-8 pass 1 не закрыт: стоит останавливать
   ticker на время `benchScroll` (и это надо оговорить в METRICS).
8. `frontend_ready_ms` = `performance.now()` при старте скрипта (`app.js:122`), т.е. до создания
   124k объектов; `METRICS.md:117` называет это «Скрипт → готовность», хотя это старт скрипта.
9. «Холодный старт до READY» измерен по строке `window-ready` из `setup()` (`main.rs:102`), т.е.
   **до** первой отрисовки webview; бюджет `ARCHITECTURE.md` §9 — «до READY». ADR-1:33 честно
   называет это «до окна», но критерий всё равно не тот (P2-4 pass 1 открыт). Риск низкий
   (137 мс против 1,5 с), но замер «до READY» отсутствует.
10. Метрики PSS/холодного старта/холодной сборки (29,56 с) не имеют артефактов в `waves/RW-36/`
    (нет `build.log`, нет лога старта) — цифры только в markdown.
11. `metrics.json` (17:25, pid 47356) и `app.pid` (17:24) устарели относительно текущего прогона —
    при перезамере (P2-2) перезаписать.
12. `screenClicks` тавтологичен: после `button.click()` кнопка всегда `.active` (`app.js:444-448`).
13. `#m-fps` после бенча живёт один кадр: `frameLoop` перезаписывает его следующим кадром
    (`app.js:132` vs `:213`); в шапке остаётся только p95.
14. `aria-live="polite"` на метриках, обновляемых 60 раз/с (`index.html:21`) — скринридер захлебнётся.
15. Кнопки рейла без `aria-label` (только `title`), эмодзи-ячейки — `div` без `role` (`index.html:12-15`).
16. У окна нет явного `"label": "main"`, при этом `capabilities/default.json:5` привязан к `"main"`
    (`tauri.conf.json:8-19`) — работает по умолчанию, связь неявная.
17. `tests/unit/gui-spike-dom.test.mjs:25-31` по-прежнему сверяет **текст исходника** регулярками
    (`/const OVERSCAN = 8/`): рефакторинг ломает тест, поведенческие дефекты не ловятся.
    Полезная замена — читать реальные значения из снимка/`app.log`.
18. `tests/unit/gui-spike-bench.test.mjs:267-294` проверяет рекурсию только на глубине 1
    (parent → child): мутант «обходить только прямых детей» выживет. Плюс `killQuietly` убивает
    только родителя, внук-`setTimeout` живёт ещё 5 с (`:270`).
19. Файловая матрица TZ §2 не содержит новых файлов `rust/gui-spike/ui/tsconfig.json` и
    `rust/gui-spike/.gitignore` — обновить матрицу (файл TL), иначе следующий md5-аудит покажет
    «незапланированные изменения».
20. `waves/RW-36` без `REPORT.md`, red/green-логов и `.gate/*.json` для batch — по §4.2/§4.7
    отчёт воркера должен их приложить.

---

## Ответы на вопросы задания

**1. Закрыты ли 4 P1?** Да, все четыре — по коду и по артефактам (таблица выше). P1-2 закрыт с
двумя оговорками (нет порога `frames`, нет `finally`) — это P2-5, не переоткрытие.
P1-3 закрыт по методике (`--tree` + PSS-заголовок в ADR), но PSS-цифра без артефакта (P2-6).

**2. Регрессии.** Новых P0/P1 нет. Проверил: гонок в `benchScroll` (guard + `settled`) не
добавлено; зависание осталось только в `nextFrame()` (P2-3); обход валидации сузился, но
rendered-строки не проверяются (P2-1); утечек нет (`rows` чистится в `paint`, `frameTimes`
ограничен 120, `rows`/`ResizeObserver`/`setInterval` живут с приложением, guard-таймер
очищается в `finish`); контракт JSON бенча совпадает с TZ §3 дословно (тест сверяет точный набор
ключей); `--tree` не двоит и не занижает молча (проверено численно).
Риск-зона: `reportUiSnapshot()` и `autoBench()` стартуют параллельно на уровне модуля
(`app.js:488-489`), синхронизации нет — на медленном software-рендере снимок может пересечься с
первым бенчем. Сейчас спасает `sleep(1200)`; надёжнее `await reportUiSnapshot(); autoBench();`
(P3).

**3. Тесты — не «зеленят» ли код?** Новые Rust-тесты проверяют поведение (типы/диапазоны/парсинг),
не реализацию; мутационный прогон `mutations-rust.log` — 7 caught / 0 survived на финальном md5
`e931bb79…`. Новые JS-тесты `parseStatus`/`percentile95`/`aggregateTree` — табличные, с
независимыми оракулами (190/95, точная сумма 350/1200/10), `spawnSync` с `timeout: 10_000`.
Слабые места: `report_bench` не тестируется вообще (P2-8), границы `parseCliArgs` (P2-13),
smoke-floor (P2-1), dom-тест по исходнику (P3-17), `chatRows`/`memberRows` в smoke берутся из тех
же констант, что и в `app.js` (тавтология; содержательная проверка — только rendered-строки).

**4. METRICS/ADR vs факты.** Числа совпадают с артефактами дословно: `metrics.json` —
26 сэмплов, avg 185 771 / max 185 784 / HWM 187 252 / threads 29; `metrics-tree.json` — 16
сэмплов, avg 586 905 / p95 588 020 / HWM-max 334 120 / threads 57 / processes 3 / skipped 0;
`app.log` — frames 300/299, avg 16.67, p95 18, fps 60, `frontend_ready_ms` 21, rendered 30/24;
все эти значения перенесены в `METRICS.md` без искажений. Подмены RSS вместо PSS **нет**: PSS
390 МБ подан отдельной строкой, RSS-сумма 573 МБ явно помечена «завышено shared-памятью», HWM
дерева — «max по процессам, не сумма». Расхождения: (а) «avg в бюджете» при 16,67 > 16 (P2-12);
(б) три прогона в одной таблице (17:25 / 18:57) без указания pid (P2-2); (в) PSS/старт/сборка
без артефактов (P2-6, P3-10); (г) `frontend_ready_ms` назван «готовностью» (P3-8).

**5. Безопасность.** `report_bench`: JSON-парсинг, обязательные объекты `chat`/`members`, печать
через `serde_json::Value` (переводы строк в строках/ключах экранируются) — инъекция `BENCH`/
`UI_SNAPSHOT` в лог закрыта; лимита размера нет (P2-8, DoS только локальный).
`report_ui_snapshot`: `as_u64` строг (строка/float/отрицательное/`null` отвергаются), добавлены
floor'ы; лишние ключи и «последний побеждает» при дублях остаются (P2 pass 1, некритично).
Capabilities/CSP не менялись: `core:default`, без fs/shell/http, `csp: null` + `withGlobalTauri`
— для спайка допустимо, `ADR-1.md:70-71` фиксирует, что в продукт это не наследуется.

**6. `--tree`.** Двойного счёта нет: `seen` + обход по `/proc/<pid>/task/*/children` (ребёнок
числится только у создавшего потока), сумма по уникальным pid. Проверено численно: 66 632 (parent)
+ 79 252 (child) = 145 884 = `rss_kb.avg` дерева, `hwm` = 79 252 (максимум, не сумма),
`threads` = 14, `skipped` = 0. Молчаливого занижения нет для штатного случая; при недоступном
`/proc` потомка сумма занижается, но это видно в `skipped` (нет только предупреждения — P3-1).
Семантика «RSS-сумма завышена shared-страницами» задокументирована и не выдаётся за footprint.

---

## Что нужно для приёмки

1. P2-2 — перезамер на финальных байтах (пересобрать, окно согласовать, переснять
   `app.log`/`metrics*.json`/`pss.log`/sha256 в `METRICS.md`). Без этого числа не сертифицируют
   ревьюнутые байты.
2. P2-1, P2-3..P2-5, P2-7..P2-9 — мелкие правки + тесты (полдня), либо осознанная отсрочка в
   `waves/RW-36/DECISIONS.md` (TZ §4.6).
3. P2-6, P2-10..P2-13 — артефакт PSS, RUNBOOK, smoke-env, формулировка бюджета, границы CLI.
4. P3 — в backlog, batch не тормозят.

**Вердикт: PASS (P0 = 0, P1 = 0).** Четыре P1 первого прохода закрыты; приёмка возможна после
закрытия/отсрочки P2 и перезамера по P2-2.
