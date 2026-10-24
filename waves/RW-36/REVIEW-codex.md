# RW-36 / B07 — код-ревью REV-1 (codex)

Дата: 2026-10-23. HEAD: `0239543`. Ревьюер: REV-1 (не автор кода).
Файлы под ревью (md5 на момент ревью):

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/main.rs` | `18c3c9e09deef9e14f4b1e09963291b6` |
| `rust/gui-spike/ui/app.js` | `51aadc8a8aeb6b38bdb22df24fe55101` |
| `rust/gui-spike/ui/index.html` | `83be4cf5ef6435e4e22447bbaf0c027a` |
| `scripts/gui-spike-bench.mjs` | `0c194f5fc494c5fe89f5aad402d79046` |
| `tests/unit/gui-spike-bench.test.mjs` | `e81f366ec9de801a739a7d16022fc55e` |
| `rust/gui-spike/Cargo.toml` | `54cfb3c1ccad25d951ae5861109cffd7` |
| `rust/gui-spike/tauri.conf.json` | `eb99adb3481255a3de282853a0c55f8c` |
| `rust/gui-spike/capabilities/default.json` | `f1cab1e9cf69fc41c55dd1106c41ac74` |

Метод: статическое чтение кода + прогон `node --test tests/unit/gui-spike-bench.test.mjs`
(13/13 pass) + `npx tsc -p tsconfig.json` (clean, но `ui/app.js` в `include` не входит — см. P2-10)
+ эмпирика из `waves/RW-36/app.log` и `waves/RW-36/metrics.json`. `cargo` не запускал
(правило «один cargo за раз», чужие target-dir не трогаю).

## Итог по приоритетам

| Приоритет | Кол-во |
|---|---|
| P0 | 0 |
| P1 | 4 |
| P2 | 11 |
| P3 | 12 |

**Вердикт: FAIL (P0 = 0, P1 = 4).** Блокеров безопасности/паник не найдено; блокируют приёмку
четыре P1 (визуальный дефект виртуализации, незащищённый авто-бенч, неполная метрика RAM для
ADR-1, неигнорируемый `target/` на 2.9 ГБ).

---

## P0 — нет

Проверял целенаправленно и не нашёл: паник на пользовательском вводе, утечек ресурсов/хендлов,
записи в файловую систему, сетевых вызовов, `unsafe`, инъекций в DOM (`innerHTML`/`eval` нет,
везде `textContent`), path traversal. `main.rs` читает только `/proc/self/status`, фронт
полностью локальный. Захват процесса/подмены PID в `gui-spike-bench.mjs` нет (только чтение,
`--pid` ограничен 1..2^22, `/proc/<pid>/status`).

---

## P1 — серьёзные (блокируют приёмку)

### P1-1. Виртуализация не перерисовывается при показе экрана и при resize → пустой список
`rust/gui-spike/ui/app.js:47-60, 71-98, 191-198, 224`

`VirtualList` красит строки только в конструкторе и по событию `scroll`. `member-list`
создаётся, когда `#screen-members` ещё `display: none` (`.screen { display:none }` в
`style.css:109-113`), значит `container.clientHeight === 0`, `scrollTop === 0`:

```js
const last = Math.min(this.items.length, Math.ceil((top + height) / this.rowHeight) + OVERSCAN);
// height = 0  →  last = 8
```

Эмпирическое подтверждение в `waves/RW-36/app.log`:
`"memberRenderedRows":8` при `"memberRows":24229`. Т.е. при переходе на «Участники 24 229»
в 820-пиксельном окне видно ~8 строк и пустоту ниже, пока пользователь не прокрутит.
Обратный эффект у чата: после `display:none` движок, как правило, сбрасывает `scrollTop` в 0
без события `scroll` (в WebKit/Blink это типичное поведение), а строки в `Map` остались от
прежней позиции (около дна) → пустой вьюпорт до первого скролла. Проверяется одним
переключением «Чат → Эмодзи → Чат» в живом окне. Плюс окно `resizable: true`, но `ResizeObserver` нет → при увеличении окна
низ списка не добивается.

DoD P0-06 («список 24 229 участников открывается») и скриншот из RUNBOOK это ловят, а
smoke-тест — нет (`assert.ok(snapshot.memberRenderedRows > 0)` проходит на 8).

**Исправление:** добавить `refresh()` (выставить `first = Math.min(first, last)` для защиты от
пустого диапазона) и вызывать его из `switchScreen()` через `requestAnimationFrame` после
добавления класса `active`, плюс `new ResizeObserver(() => this.schedule()).observe(container)`
в конструкторе. В smoke-тесте проверять `memberRenderedRows >= 15` (реальная ёмкость вьюпорта),
а не `> 0`.

### P1-2. `benchScroll` без таймаута: при остановленном rAF авто-замер молча не завершается
`rust/gui-spike/ui/app.js:143-173, 355-379`

Промис резолвится только из `requestAnimationFrame`. Если окно occluded/minimized, композитор
на Wayland не отдаёт кадры (типичный сценарий headless-прогона) — `step` не вызывается,
`await` висит вечно, `benchRunning` навсегда `true`, `report_bench` не вызывается, строка
`BENCH ...` в `app.log` отсутствует. Диагностики нет: наружу это выглядит как «метрики просто
не снялись», т.е. ошибка проглочена (нарушение правила AGENTS.md «не глотать молча»).

Там же крайний случай вырожденной выборки: если кадров 1, `deltas.shift()` опустошает массив →
`avg = 0/1 = 0` → `fps = 1000/0 = Infinity` → `JSON.stringify` даёт `"fps": null`, а
`p95.toFixed(1)` на 0 формально ок, но результат бессмысленный.

**Исправление:** сторож `setTimeout(() => { clearTimeout(timer); benchRunning = false;
reject(new Error("rAF не тикал за durationMs")); }, durationMs + 2000)`, `benchRunning = false`
в `finally`, и валидация `frames >= 30` с явным `{ error: "no-frames" }` в payload вместо
`Infinity`. В `autoBench` — проверять результат и вызывать `report_bench` с полем `error`,
чтобы прогон без FPS был видимым провалом, а не пропуском.

### P1-3. Метрика RAM неполная: меряется только родительский процесс, WebKit — нет
`scripts/gui-spike-bench.mjs:136-152, 158-174`, `rust/gui-spike/src/main.rs:26-36`,
`waves/RW-36/RUNBOOK.md` («RAM в простое ≤ 300 МБ»)

Контракт ТЗ §3 заморожен на `/proc/<pid>/status`, и код ему соответствует. Но на Linux Tauri
порождает `WebKitWebProcess` (и `WebKitNetworkProcess`) как **отдельные процессы** со своим
адресным пространством: `VmRSS`/`VmHWM` родителя их не включают. `waves/RW-36/metrics.json`
даёт `rss_kb.avg = 185 771` (181 МБ) — это только оболочка; реальный стек ближе к 250–400 МБ.
Критерий «≤ 300 МБ» и сравнение с альтернативами в ADR-1 могут быть выполнены/провалены на
неверном числе, а это и есть основной артефакт волны.

**Исправление (нужно решение TL, контракт ТЗ не мой):** либо добавить в скрипт режим
`--tree` (сумма RSS по дереву: `/proc/<pid>/task/*/children` рекурсивно, либо один проход по
`/proc/*/stat` с `ppid`), либо явно зафиксировать в `ADR-1`/`METRICS.md`, что цифра —
per-process, а не per-stack, и привести сравнение к одной методике. Без этого ADR-1 на этих
числах защитить нельзя.

### P1-4. `rust/gui-spike/target/` (2.9 ГБ) не в `.gitignore` и попадёт в коммит
`.gitignore` (весь файл; игнорируется только `waves/**/target/`), `rust/gui-spike/`

`git status` показывает `?? rust/gui-spike/` целиком; `du -sh rust/gui-spike/target` = 2.9 ГБ.
`target/` (плюс генерируемый `gen/`, 304 КБ) не игнорируется, значит `git add rust/gui-spike`
утащит бинарники и артефакты сборки в репозиторий — это дорого/необратимо (history bloat).

**Исправление:** TL добавляет в `.gitignore` строки `rust/gui-spike/target/` и
`rust/gui-spike/gen/` (файл общий, правит только TL). `Cargo.lock` при этом **оставить** в
индексе — для бинарного крейта он нужен для офлайн-воспроизводимости.

---

## P2 — средние

### P2-1. `report_bench` принимает произвольную строку → подделка строк в `app.log`
`rust/gui-spike/src/main.rs:46-49`

```rust
fn report_bench(payload: String) { println!("BENCH {payload}"); }
```

Никакой валидации: payload с `\n` позволяет вписать в лог что угодно, включая
`UI_SNAPSHOT {...}` (харнесс/smoke-тест ищут строку по префиксу) и любые `BENCH` с
произвольными цифрами. Т.е. замеряемый код сам формирует доказательство приёмки. Вектора
извне нет (фронт локальный), поэтому не P0/P1, но контракт «снимок валидируется бэкендом»
здесь не выполняется вообще, в отличие от `report_ui_snapshot`.
**Исправление:** парсить payload как JSON (`serde_json::from_str::<serde_json::Value>`), печатать
компактно (`{value}`), отклонять строки с `\n`/`\r`, ограничить длину (например, 4 КиБ) и
возвращать `Err`.

### P2-2. `validate_ui_snapshot`: типы проверяются, значения — нет
`rust/gui-spike/src/main.rs:51-60`

Разбор: `as_u64()` действительно строг по типу — проходит только неотрицательное целое;
строка `"100000"`, `4.0`, `-1`, `true`, `null` отвергаются. Но:
- нет проверки диапазонов/согласованности: `{"screenCount":0,"chatRows":1,"memberRows":0,
  "emojiCategories":0,"settingsToggles":0}` проходит валидацию и печатается как валидный снимок;
- лишние ключи разрешены (payload любого размера попадает в лог);
- `screenCount` в тесте ожидается равным 4, но валидатор это не проверяет — проверка живёт
  только в smoke-тесте, а он опционален (`GUI_SPIKE_E2E=1`);
- дубликаты ключей в JSON: побеждает последний.
**Исправление:** `#[derive(Deserialize)] #[serde(deny_unknown_fields)] struct UiSnapshot
{ screen_count: u64, chat_rows: u64, ... }` + проверки ожидаемых значений (`== 4`, `>= 100_000`,
`== 24_229`, `== 10`, `== 38`) или, как минимум, границы `>= 1` и `chat_rows < CHAT_ROWS`.

### P2-3. `process_metrics` глушит отсутствие полей через `unwrap_or(0)`
`rust/gui-spike/src/main.rs:31-35`

`0` неотличим от реального значения и от ошибки чтения; UI покажет «0 МБ» вместо «err».
**Исправление:** `read_status_field(...).ok_or_else(|| format!("нет поля {key}"))?` — вернуть
`Err`, фронт уже умеет показывать `err`.

### P2-4. Маркер `window-ready` печатается до первой отрисовки
`rust/gui-spike/src/main.rs:71-83`

`setup()` вызывается при сборке приложения; печать идёт до того, как webview отрисовал
frontend (в том же `app.log` следом идут `Failed to create GBM buffer`). RUNBOOK использует эту
строку как «окно создано» и как точку отсчёта холодного старта — метрика холодного старта
неверна. **Исправление:** печатать маркер из фронта (первый `requestAnimationFrame` после
первого `paint`) через отдельную команду `report_ready`, либо назвать строку честно
(`window-created`).

### P2-5. Тесты `main.rs` не ловят подмену проверки типа
`rust/gui-spike/src/main.rs:94-122`

Мутант «заменить `and_then(as_u64).is_none()` на `get(key).is_none()`» (проверка только
наличия ключа) выживает: `validates_ui_snapshot_contract` смотрит лишь `value["screenCount"]`,
`rejects_incomplete_ui_snapshot` — только отсутствие `chatRows`. Нет теста «строка вместо
числа», «отрицательное», «дробное», «неверный диапазон».
**Исправление:** кейсы `#[test] rejects_string_typed_field()` (`{"chatRows":"100000",...}` →
`Err`), `rejects_negative_and_float()`, `rejects_out_of_range()` (после P2-2), и проверка
`assert_eq!(value["chatRows"], 100_000)` вместо одного поля.

### P2-6. Два разных p95: UI считает `floor(n*0.95)`, скрипт — nearest-rank
`rust/gui-spike/ui/app.js:165` vs `scripts/gui-spike-bench.mjs:54-58`

```js
const p95 = sorted[Math.floor(sorted.length * 0.95)] ?? 0;   // app.js
const rank = Math.ceil(0.95 * sorted.length); return sorted[Math.max(0, rank - 1)] ?? 0; // mjs
```

Для n=100 это 96-е значение против 95-го, для n=20 — максимум против 19-го. Числа FPS/p95 в
шапке и в `metrics.json` несравнимы, а порог приёмки «p95 кадра ≤ 20 мс» считается по более
пессимистичной (завышенной) формуле. **Исправление:** вынести `percentileNearestRank(sorted, p)`
в общий модуль (или продублировать формулу с комментарием) и использовать одну реализацию.

### P2-7. `frontend_ready_ms` меряет конец прогона, а не готовность фронта
`rust/gui-spike/ui/app.js:366-372`

`performance.now()` берётся после `sleep(1200)` + двух 5-секундных бенчей, т.е. значение
~11 500 мс попадает в `BENCH` под именем «frontend_ready_ms». Это прямо искажает ADR-1
(время готовности UI). **Исправление:** зафиксировать `const readyAt = performance.now()` в
первом `requestAnimationFrame` (или сразу после `reportUiSnapshot`) и передавать его; поле
переименовать в `bench_finished_ms`, если нужен конец.

### P2-8. Метрики шумят сами себя + `el()` маскирует `null`
`rust/gui-spike/ui/app.js:108, 111-119, 121-138`

`frameLoop` пишет `textContent` в DOM каждый кадр, `pollProcessMetrics` делает IPC каждые 2 с —
и то и другое идёт **во время** измерения кадров, добавляя layout/IPC-спайки в те же
`deltas`, по которым считается p95. `el()` кастует `HTMLElement` без проверки: опечатка в id
даёт `TypeError: Cannot set properties of null` вместо внятной ошибки.
**Исправление:** на время `benchScroll` останавливать `frameLoop`/`pollProcessMetrics` (или
считать p95 по кадрам без DOM-записи), в `el()` — `throw new Error("нет элемента #"+id)`.

### P2-9. Тест покрывает только `summarize`; `parseStatus`/`parseCliArgs` — нет
`tests/unit/gui-spike-bench.test.mjs:13, 35-132`

ТЗ §3 требует тест `summarize`, и он корректен (p95 nearest-rank на n=20/n=100, пустой вход,
один элемент, отсутствие мутации входа, округление avg — проверено прогоном, 13/13). Но:
- `parseStatus` (реальный парсер `/proc`, `scripts/gui-spike-bench.mjs:111-130`) не покрыт
  ничем → мутанты «сделать ` kB` обязательным», «`\d+` → `[-+]?\d+`», «выкинуть проверку
  пропавших полей» выживают;
- `parseCliArgs` не покрыт по границам: мутант «убрать `value > max`» (pid > 2^22) выживает,
  дубли флагов не проверяются;
- нет кейса «при exit 2 файл `--json` не создан» и «stdout пуст при ошибке»;
- нет кейса n=2 / n=19, где nearest-rank даёт максимум;
- `spawnSync` без `timeout` — зависший скрипт подвесит тест-раннер.
**Исправление:** добавить табличные тесты `parseStatus` (валидный статус, отсутствие `VmHWM`,
мусор) и `parseCliArgs` (`--pid 0`, `--pid 4194305`, дубль `--pid`, `--json ""`), плюс
`timeout: 10_000` в `runCli`.

### P2-10. `ui/app.js` не попадает в typecheck, `// @ts-check` декоративен
`tsconfig.json:19-27` (`include` без `rust/**`), `rust/gui-spike/ui/app.js:1`

Гейт `npm run typecheck` проверяет `scripts/**`, `src/**`, `bin/**`, `tests/**` — но не
`rust/gui-spike/ui/app.js`. Проверил принудительно:
`npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict --lib ES2023,DOM rust/gui-spike/ui/app.js`
даёт ~20 ошибок (неявные `any` в `scrollToIndex`/`switchScreen`/`sleep`, `Object is possibly
'null'` на `app.js:197`, `Property 'click' does not exist on type 'Element'` на `app.js:386`).
Отдельно: `let frames = []` (`app.js:111`) в классическом (не module) скрипте затеняет
`window.frames` — сейчас работает, но это мина под будущий рефакторинг.
**Исправление:** либо `tsconfig.gui.json` с `lib: ["ES2023","DOM"]` и включение его в
`typecheck` (и тогда починить JSDoc), либо убрать `// @ts-check` из `app.js`, чтобы не создавать
иллюзию проверки. `frames` → `frameTimes`.

### P2-11. RUNBOOK не документирует авто-бенч и границы метрик
`waves/RW-36/RUNBOOK.md`

В коде есть `SPIKE_AUTOBENCH` (`main.rs:38-43`, `app.js:355-379`), строка `BENCH` и
контракт `UI_SNAPSHOT`, smoke-тест их использует — в RUNBOOK этого нет (только ручная кнопка).
Также не сказано, что RAM — per-process (см. P1-3). **Исправление:** дописать раздел
«авто-прогон» с переменными `SPIKE_AUTOBENCH`, `SPIKE_EXIT_AFTER_MS`, форматом `BENCH`/
`UI_SNAPSHOT` и оговоркой о scope метрики.

---

## P3 — мелкие

1. `scripts/gui-spike-bench.mjs:253-260` — ошибка записи `--json` (в т.ч. путь-каталог)
   отдаёт exit 2; по ТЗ это ошибка аргументов → логичнее exit 1 (или валидировать путь до замера).
2. `scripts/gui-spike-bench.mjs:203-219` — дубли флагов молча перезаписываются
   (`--pid 1 --pid 2`), `--help` нет; стоит отвергать повторы.
3. `scripts/gui-spike-bench.mjs:266-270` — запуск через симлинк не сработает
   (`resolve(argv[1])` vs `fileURLToPath`): скрипт молча завершится с exit 0, ничего не сделав.
   Использовать `realpathSync`.
4. `scripts/gui-spike-bench.mjs:114` — имя поля не экранируется в `RegExp` (сейчас безопасно,
   поля-константы) и `\s+` формально может пересекать перевод строки; лучше
   `new RegExp("^" + name + ":[ \\t]+(\\d+)")`.
5. `rust/gui-spike/src/main.rs:74-80` — невалидный `SPIKE_EXIT_AFTER_MS` (`"abc"`) игнорируется
   молча; стоит напечатать предупреждение в stderr.
6. `rust/gui-spike/src/main.rs:13` `rss_kib` vs `scripts/...:16` `rss_kb` — разнобой имён в
   одном контракте.
7. `rust/gui-spike/src/main.rs:91` — `expect` печатает панику без кода выхода; для спайка ок,
   но harness не отличит «нет дисплея» от «упало».
8. `rust/gui-spike/ui/app.js:168-169` — результат бенча в шапке живёт один кадр: `frameLoop`
   сразу перезаписывает `#m-fps`, остаётся только `p95`; строка `[bench]` есть только в консоли.
9. `rust/gui-spike/ui/app.js:384-388` — `screenClicks` всегда равен числу кнопок (проверка
   тавтологична после `switchScreen`); лучше считать реальные переключения `#screen-*.active`.
10. `rust/gui-spike/ui/index.html:21` — `aria-live="polite"` на метриках, обновляемых 60 раз/с:
    скринридер будет захлёбываться. Убрать `aria-live` или обновлять раз в секунду.
11. `rust/gui-spike/ui/index.html:12-15, 287-292` — кнопки рейла без `aria-label` (только
    `title`), эмодзи-ячейки — `div` без `role="button"`/подписи.
12. `rust/gui-spike/tauri.conf.json:10-18` — у окна нет явного `"label": "main"`, хотя
    `capabilities/default.json:5` привязывается к `"main"` (работает по умолчанию, но связь
    неявная); `bundle.icon: []` при наличии `icons/icon.png`.

---

## Ответы на заданные вопросы

**Можно ли обойти `validate_ui_snapshot`?** Типы — да, проверяются (только `u64`; строки,
float, отрицательные, `null` отвергаются). Значения — нет: `screenCount: 0`, `chatRows: 1`,
`settingsToggles: 0` проходят (P2-2). Лишние ключи разрешены, дубликаты ключей — «последний
побеждает». Вторая, более широкая дыра — `report_bench` вообще без валидации и с `\n`-инъекцией
в лог (P2-1). Итог: как «эхо-проверка наличия полей» валидатор работает, как защита
доказательства приёмки — нет.

**Виртуализация.** Границы `first/last` считаются верно, включая пустой список (`height=0`,
`last=0`, циклы пустые) и `scrollToIndex` за пределами (scrollTop клампится браузером).
Утечек DOM нет: `paint()` удаляет узлы вне окна и чистит `Map`, `sizer` один. Проблемы — не в
границах, а в отсутствии перерисовки при показе/resize (P1-1) и в потенциальном `first > last`
при изменении `items` (не достигается, но `Math.min(first, last)` — дешёвая страховка).

**`gui-spike-bench.mjs`.** p95 nearest-rank реализован корректно (`ceil(0.95n)-1`, кламп в 0,
пустой вход → 0) и совпадает с тестом (n=20 → 190, n=100 → 95). `parseStatus` строг и
устойчив к мусору: три обязательных поля, при пропаже — `Error` с перечнем, наверх это
превращается в exit 2. Коды выхода соответствуют ТЗ §3: 1 — аргументы, 2 — `/proc`/процесс/
запись, 0 — успех; частичный выход процесса посреди замера тоже даёт 2 (валидно). Замечания —
P2-9 (нет тестов парсера/аргументов) и P3-1..4.

**Тест.** Контракт `summarize` покрыт хорошо (включая пустой вход, порядок, отсутствие
мутации), p95 — на двух размерах. Слабые места: `parseStatus`/`parseCliArgs` не покрыты вообще,
типы в `validate_ui_snapshot` не проверяются тестом (P2-5), а `tests/unit/gui-spike-dom.test.mjs`
(вне матрицы ревью) сверяет **текст исходника** регулярками (`/const OVERSCAN = 8/`) — это
подгонка под реализацию, а не контракт: ни один из этих тестов не поймал бы P1-1.

**Tauri capabilities.** Минимальны и корректны по форме: `windows: ["main"]`, `permissions:
["core:default"]`, файловой системы/сети/шелла нет, `bundle.active: false`. Строго говоря,
прикладные команды (`#[tauri::command]`) ACL не требуют, так что хватило бы и пустого набора;
`core:default` — документированный baseline, лишнего вреда не несёт (P3). Отдельно: нет
`assetProtocol`/`dangerousRemoteDomainIpcAccess` — хорошо.

**CSP `null` + `withGlobalTauri: true`.** Для спайка допустимо: контент только локальный,
сеть не используется, весь пользовательский текст выводится через `textContent`, инъекции нет,
команд всего четыре и они read-only (кроме печати в stdout). Риск появится в момент, когда в
окно попадёт хоть какой-то удалённый/пользовательский HTML: `csp: null` + глобальный
`__TAURI__` превратят любую XSS в полный доступ к IPC. Рекомендация: до переноса в продукт
зафиксировать в ADR-1 как условие (`"csp": "default-src 'self'; connect-src ipc:
http://ipc.localhost"` и `withGlobalTauri: false` с `@tauri-apps/api`), и не наследовать эти
две строки в продуктовый конфиг.

---

## Наблюдения вне матрицы ревью (для TL, не блокируют)

1. `rust/gui-spike/target/` 2.9 ГБ и `gen/` — см. P1-4, правит `.gitignore` только TL.
2. `tests/smoke/gui-spike.smoke.test.mjs:47-51` — `chatRows`/`memberRows` в снимке и в тесте
   берутся из одних и тех же констант `app.js`, т.е. проверка тавтологична; фактическое
   доказательство виртуализации — только `chatRenderedRows`/`memberRenderedRows`, и их нижняя
   граница сейчас `> 0` (пропускает P1-1).
3. `tests/unit/gui-spike-dom.test.mjs` — регулярки по исходнику `app.js`; любой рефакторинг
   ломает тест без изменения поведения, а поведенческие дефекты не ловятся.
4. В `waves/RW-36/` нет red/green-логов для `gui-spike-bench` (только `app.log`, `app.pid`,
   `metrics.json`, `RUNBOOK.md`, `TZ.md`) — по §4.2 отчёт W36b должен их приложить; проверить
   red-first по факту прогона, а не по заголовку теста.
5. `app.log` содержит `Failed to create GBM buffer ... Недопустимый аргумент` (Wayland), при
   этом smoke-тест требует отсутствия этой строки в stderr — на живом окружении он может
   падать/флакать; флаги `WEBKIT_DISABLE_DMABUF_RENDERER=1` в smoke есть, в RUNBOOK — нет.

---

## Что нужно для PASS

1. P1-1: `refresh()` + вызов при `switchScreen`/`ResizeObserver`, нижняя граница rendered-строк
   в smoke-тесте.
2. P1-2: таймаут/`finally` в `benchScroll`, явный `error` в payload вместо `Infinity`.
3. P1-3: решение TL по scope RAM (сумма по дереву процессов **или** оговорка в ADR-1/METRICS).
4. P1-4: `rust/gui-spike/target/` (+`gen/`) в `.gitignore`.
5. P2 закрыть или осознанно отложить с записью в `waves/RW-36/DECISIONS.md` (по критерию 6 ТЗ
   P2 можно отложить, P1 — нет); после фиксов — повторный batch gate и повторное ревью на
   финальных байтах (md5 в отчёте переснять).
