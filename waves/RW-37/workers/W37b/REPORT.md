# RW-37 / W37b — отчёт воркера: оконная/постраничная модель данных (ADR-1)

- batch/карточка: RW-37 (foundation-условие ADR-1; вне счётчика 90).

## m09 (QA-1, повторный прогон) — перегрузка страницы

- **Пробел:** мутация `rows.length !== capacity` → `rows.length < capacity` (перегрузка
  страницы разрешена) выживала — не было кейса «fetchPage вернул больше capacity».
- **Тест добавлен** (`tests/unit/gui-spike-model.test.mjs`, describe «ensureRange»):
  не-последняя страница отдаёт 101 строку при capacity 100 → `RangeError`
  (`fetchPage(0)`, «ожидалось 100», «получено 101»), `loadedPages/loadedRows` не растут,
  повторный `ensureRange` после ошибки успешен (`getRow(99)` на месте). Вторая половина
  кейса — последняя страница: capacity = остаток (50), 51 строка → `RangeError`
  (`fetchPage(2)`, «ожидалось 50», «получено 51»), кэш пуст.
- **Red-first мутация m09** (`!==` → `<`): `node --test --test-name-pattern="перегрузка
  страницы" …` → **1 fail**, `Missing expected rejection` (`red-m09.log`, exit 1) — мутант
  KILLED. Код восстановлен: `model.js` md5 `ccdbb0a4c7e407aaa16f824ee4eeffe9` (совпадает
  с md5 до мутации), тот же тест → 1 pass.
- **Финал:** targeted 3 файла — **66/66 pass** (`green.log`); `tsc` UI — 0.
- `model.js` не менялся (код верен); правился только тест-файл.

### Файлы m09 (md5)

| Файл | md5 |
|---|---|
| `tests/unit/gui-spike-model.test.mjs` | `8580f717aab812215cc8101b8cf7123a` |
| `rust/gui-spike/ui/model.js` (не изменён) | `ccdbb0a4c7e407aaa16f824ee4eeffe9` |
| `waves/RW-37/workers/W37b/red-m09.log` | `7930d8b0cd3b2ec756c7fac87cbf32ea` |
| `waves/RW-37/workers/W37b/green.log` | `c721b5ff4e676b1ca5819336eaaeeb23` |
| `waves/RW-37/workers/W37b/checks.log` | `057686b6da4aa2cfc87310131cb42e4c` |

## P2 rework (REV-1, PASS с 2×P2)

- **P2-1 (app.js) — placeholder не очищал детей.** `renderChatRow`/`renderMemberRow`
  начинаются с `row.className = <base>` (снимает `placeholder`) и `row.replaceChildren()`;
  placeholder-ветка оставляет элемент без детей — только фон `.placeholder`. Переходы
  `dataset.state` (`row`↔`placeholder`) вызывают render только при смене состояния, а
  render теперь полностью пересобирает содержимое → дублей/унаследованного текста нет.
  Живой smoke на финальном бинаре: **1/1 pass**, `UI_SNAPSHOT` (rendered 30/24,
  loaded 400/100, `actions` все true) не изменился.
- **P2-2 (model.js) — пустая последняя страница принималась навсегда.** Red-first:
  тест «последняя страница вернула 0 строк при capacity>0 → RangeError» (`red-p2.log`:
  0 pass / **1 fail**, exit 1 — промис разрешался). Фикс в `loadPage`: единая проверка
  точной вместимости `capacity = min(pageSize, totalRows - pageIndex*pageSize)` для **всех**
  страниц; `rows.length !== capacity` → `RangeError` с pageIndex, capacity и фактическим
  размером. Перегрузка страницы теперь покрыта той же проверкой (отдельная ветка `>` убрана
  как дублирующая). Последняя неполная страница с ровно `capacity` строками валидна:
  кейсы «250 строк → 50 в третьей» и «101 строка → 1 в последней» остались зелёными.
- **Проверки:** targeted 3 файла — **65/65 pass** (`green.log`); `tsc` UI — 0; `tsc` root — 0;
  `cargo build --offline` — ok; live smoke — 1/1 (`checks.log`).

### Файлы P2 rework (md5)

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/model.js` | `ccdbb0a4c7e407aaa16f824ee4eeffe9` |
| `rust/gui-spike/ui/app.js` | `32ed027bc8cf5c72f5a0ca4705706eab` |
| `tests/unit/gui-spike-model.test.mjs` | `91dd3c72e861896a2ea146fc936996e5` |
| `waves/RW-37/workers/W37b/red-p2.log` | `c6accfb2ba891820831d802ad58817d2` |
| `waves/RW-37/workers/W37b/green.log` | `bdd54148f11e3ed8a20f5abecdf6cf64` |
| `waves/RW-37/workers/W37b/checks.log` | `075f915196c7e09a7172e938a3fef5f6` |

## m07 (QA-1) — закрытие пробела покрытия LRU

- **Пробел:** `evict` мог игнорировать `pinned` и вытеснять страницу видимого диапазона
  при конкурентных `ensureRange`; существующие LRU-тесты этого не ловили, потому что
  запиненная страница в них и так была свежее по доступу.
- **Тест добавлен** в `tests/unit/gui-spike-model.test.mjs` (describe «LRU-эвикция и
  бюджет»): `maxPages=2`, `pageSize=10`, `ensureRange(0,9)` → page 0 в кэше; конкурентно
  `ensureRange(0,19)` (page 1 резолвится через 60 мс) и `ensureRange(20,29)` (page 2 —
  через 10 мс); после `Promise.all` — `loadedPages=2`, `getRow(0)`/`getRow(19)` на месте.
  Сценарий перенесён из `/tmp/opencode/qa1-m07-candidate.test.mjs`, зависимость от `/tmp`
  отсутствует.
- **Red-first мутация m07** (временно `pinned.has(pageIndex)` → игнор):
  `node --test --test-name-pattern="m07" tests/unit/gui-spike-model.test.mjs` →
  **1 fail**, `actual: undefined`, `expected: { id: 0 }` (`red-m07.log`, exit 1) — мутант
  KILLED. Код восстановлен: `model.js` md5 `8f0e4d19d7654a97b478acff9a95b909` (совпадает
  с md5 до мутации), тот же тест → 1 pass.
- **Финал:** targeted 3 файла — **64/64 pass** (`green.log`); `tsc` UI — 0; `tsc` root — 0.
- `model.js` по существу не менялся (код был верен); правился только тест-файл.

### Файлы m07 (md5)

| Файл | md5 |
|---|---|
| `tests/unit/gui-spike-model.test.mjs` | `0b68e39ecf2f8541f85fb6a88ba51de9` |
| `rust/gui-spike/ui/model.js` (не изменён) | `8f0e4d19d7654a97b478acff9a95b909` |
| `waves/RW-37/workers/W37b/red-m07.log` | `0336dfdb8d7a60477d3c62d7b8a64be7` |
| `waves/RW-37/workers/W37b/green.log` | `465cc6c1b162e19ed8b718f07f2c0f5d` |
| `waves/RW-37/workers/W37b/checks.log` | `412816d00902ca010cdf7855f86916b8` |

## Rework по приёмке PM (2×P2 + 1×P3)

- **R1 (P2) — короткая не-последняя страница.** Red-first: добавлены тесты
  «короткая не-последняя страница → RangeError» и «последняя (неполная) страница остаётся
  допустимой» (`red-r1.log`: 22 pass / **1 fail** — падал новый тест, exit 1). Фикс в
  `model.js::loadPage`: если `pageIndex < pageCount - 1 && rows.length !== pageSize` →
  `RangeError` с `fetchPage(<индекс>)`, ожидаемым и фактическим числом строк; битая страница
  не попадает в кэш, `inFlight` очищается (повтор возможен). Последняя короткая страница
  (50 из 100, 1 из 100) — допустима, покрыто тестом. Green: 23/23.
- **R2 (P2) — статический ES-модуль.** `index.html` → `<script type="module" src="app.js">`;
  в `app.js` статический `import { PageStore } from "./model.js";` (первая строка после
  docblock), `modelModule` и динамический `import()` удалены; `initRowLists()` стал
  синхронным, `listsReady`/`await` убраны (module-скрипты defer, DOM готов). Проверено живым
  прогоном: `GUI_SPIKE_E2E=1 node --test tests/smoke/gui-spike.smoke.test.mjs` → **1/1 pass**
  (`checks.log`), `UI_SNAPSHOT`/`BENCH` не изменились.
- **R3 (P3) — placeholder.** В `style.css` добавлено `.placeholder { background: var(--bg-2);
  opacity: .45; border-bottom-color: transparent; }`; `renderChatRow`/`renderMemberRow` больше
  не пишут текст в placeholder — только скелетон. Классы `.row`/`.member` сохранены, поэтому
  `memberRenderedRows` в снапшоте по-прежнему считает placeholder-строки.
- **PSS после rework:** 282 918 КиБ ≈ **276,3 МБ** и 284 286 КиБ ≈ **277,6 МБ** (два прогона
  на финальных файлах), `WebKitWebProcess` 164,1/161,5 МБ; `BENCH` — 60 fps, avg 16,67 мс,
  p95 18 мс, `loadedRows` 400/100. Было до rework 287,9/298,2 МБ, в RW-36 — 393,9 МБ.
- **Проверки:** targeted 3 файла — **64/64 pass** (`green.log`, после теста m07); `tsc` UI и root — 0;
  `node --check` app/model — 0; `cargo build --offline` + `cargo test --offline` — 16 passed;
  live smoke — 1/1; всё в `checks.log`.

### Итоговые файлы rework (md5)

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/model.js` | `8f0e4d19d7654a97b478acff9a95b909` |
| `rust/gui-spike/ui/app.js` | `79a5c6d64c8989a493b2083023412015` |
| `rust/gui-spike/ui/index.html` | `d2ca86ad0a798854aa2c609697ae6dba` |
| `rust/gui-spike/ui/style.css` | `f944ef11bfed75cd38116df9b563a4c8` |
| `tests/unit/gui-spike-model.test.mjs` | `1fdcbdad1a4eaf9bf349fa6fe4843481` |
| `waves/RW-37/workers/W37b/red-r1.log` | `bcd3bbeb7c2e00a2e74e8f100fc6d68e` |
| `waves/RW-37/workers/W37b/green.log` | `12da906386faa2be18e90be64f6ff6fa` |
| `waves/RW-37/workers/W37b/checks.log` | `8dd8faa8c622425c7dfce9098cb9da4f` |
| `waves/RW-37/workers/W37b/pss.json` | `32c41bd44dd4e1a840c8a092f895da3c` |
| `waves/RW-37/workers/W37b/app.log` | `d2ce4e82ce6db678769e89d8c09671de` |

- **сделано**:
  - `rust/gui-spike/ui/model.js` (новый, ESM) — `PageStore`: страницы по `pageSize`,
    LRU-эвикция по последнему доступу с пином текущего диапазона, кэш промисов загрузки
    (`inFlight`) против двойных загрузок, `getRow`, `ensureRange`, `stats`.
    Валидация в конструкторе (`TypeError` на нецелые/≤0 `totalRows`/`pageSize`/`maxPages`,
    отсутствующий `fetchPage`); `RangeError` на диапазон шире `maxPages`; `TypeError` на
    не-массив из `fetchPage` и на нецелые аргументы `ensureRange`. Ошибки `fetchPage` не
    глотаются: промис отклоняется, из `inFlight` удаляется — повторная загрузка возможна.
  - `tests/unit/gui-spike-model.test.mjs` (новый) — 26 тестов (4 сьюта; 21 в первой сдаче +
    2 red-first на R1 + 1 на m07 + 1 на P2-2 + 1 на m09): конструктор и
    валидация, одна строка, пустой стор, границы первой/последней (неполной) страницы,
    клампы диапазона, no-op вне данных, идемпотентность, конкурентный `ensureRange`
    (delay 10 мс, ровно один fetch на страницу), ошибка+повтор, не-массив, диапазон шире
    бюджета, и LRU (3 кейса), инвариант `loadedRows ≤ pageSize*maxPages` при обходе 11
    страниц, `stats`.
  - `rust/gui-spike/ui/app.js` — секции чата/участников и модель:
    `CHAT_PAGE_SIZE/MEMBER_PAGE_SIZE = 100`, `*_MAX_PAGES = 5` (≤500 строк на список вместо
    124 229); `makeMessage`/`makeMember`/`makePage` вместо материализации массивов;
    `VirtualList` работает от `RowSource` (`totalRows/getRow/ensureRange`), на `undefined`
    рисует placeholder и не блокирует скролл (`requestWindow` → `ensureRange`, перерисовка
    только если данные реально пришли — иначе был бы вечный rAF-цикл; ошибка `ensureRange`
    и «пустое окно» логируются); `UI_SNAPSHOT` дополнен аддитивными полями
    `chatLoadedRows`/`memberLoadedRows` (окно в JS, а не только DOM), остальные поля и
    `BENCH` не изменены. После rework R2 модель подключается статическим
    `import { PageStore } from "./model.js"`, `index.html` — `<script type="module">`,
    `initRowLists()` синхронный (module-скрипты defer, DOM уже разобран).

- **red**: `node --test tests/unit/gui-spike-model.test.mjs` → `ERR_MODULE_NOT_FOUND`
  `rust/gui-spike/ui/model.js`, exit 1 (лог `waves/RW-37/workers/W37b/red.log`);
  rework R1 — `red-r1.log`: 22 pass / **1 fail** (новый тест «короткая не-последняя
  страница → RangeError»), exit 1.

- **green**: `node --test tests/unit/gui-spike-model.test.mjs` → 23/23 pass, exit 0
  (`green.log` — прогон 3 файлов: 66/66 pass, exit 0).

- **targeted**: `node --test tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs
  tests/unit/gui-spike-dom.test.mjs` → 63 pass / 0 fail (лог `green.log`);
  `GUI_SPIKE_E2E=1 node --test tests/smoke/gui-spike.smoke.test.mjs` → 1/1 pass;
  `npx tsc -p rust/gui-spike/ui/tsconfig.json` → exit 0;
  `npx tsc -p tsconfig.json` (root, включает тесты) → exit 0;
  `node --check` для `app.js` и `model.js` → exit 0;
  `cd rust/gui-spike && cargo build --offline` → ok; `cargo test --offline` → 16 passed
  (лог `checks.log`). Модельные тесты прогонялись многократно подряд — стабильно зелёные.

- **oracle**: независимый оракул — арифметика страниц (`ceil(totalRows/pageSize)`,
  границы `page*pageSize … min(total, +pageSize)`) и вручную выведенный порядок LRU в
  тестах; тесты не читают внутренние поля `PageStore` (`pages`/`clock`), только
  `getRow`/`ensureRange`/`stats` и журнал вызовов `fetchPage`. Живой замер — независимый
  артефакт окружения: `pss.json`/`metrics*.json` (`waves/RW-36/measure-pss.py`,
  `scripts/gui-spike-bench.mjs`).

- **генерация**: не применимо (строки генерируются детерминированно по индексу,
  `makeMessage(i)`/`makeMember(i)`).

- **мутации**: не выполнялись — по матрице ТЗ это QA-1 (`waves/RW-37/run-mutations*.py`).

- **baseline/restore**: не применимо (мутаций не было; файлы RW-36 не изменялись).

- **стабильность**: конкурентный кейс `ensureRange` стабилен (delay 10 мс, повторные
  прогоны зелёные); живые прогоны (smoke + два замера) прошли без ошибок в stderr.

- **Живой предварительный замер (до rework; после rework — см. раздел выше)**:
  `python3 waves/RW-37/workers/W37b/live-measure.py --wait-s 40`
  (адаптация RW-36 `remeasure.py`, файл RW-36 не изменялся), два прогона на финальном
  `app.js`:
  - `UI_SNAPSHOT {"chatLoadedRows":400,"chatRenderedRows":30,"chatRows":100000,
    "memberLoadedRows":100,"memberRenderedRows":24,"memberRows":24229,"screenCount":4,
    "emojiCategories":10,"settingsToggles":38,"actions":{screenClicks:4,categoryClicks:true,
    toggleClick:true,listScroll:true}}`;
  - `BENCH {"chat":{"fps":60,"avgMs":16.67,"p95Ms":18,"frames":300},
    "members":{"fps":60,"avgMs":16.67,"p95Ms":18,"frames":300},"frontend_ready_ms":21}`;
  - **PSS дерева: 294 810 КиБ ≈ 287,9 МБ** (прогон 1) и **305 333 КиБ ≈ 298,2 МБ**
    (прогон 2); было 403 328 КиБ ≈ 393,9 МБ в RW-36. По процессам (прогон 2): main 80,9 МБ,
    `WebKitWebProcess` **185,9 МБ** (было 281,6), `WebKitNetworkProcess` 31,4 МБ.
    Бюджет ARCHITECTURE §9 (≤300 МБ) выполнен в обоих прогонах, но запас во втором
    ≈1,8 МБ — разброс между прогонами ≈10 МБ, финальное число за PM.

- **gate:worker**: не запускал (batch-гейт — у Integration/QA), targeted-набор и `tsc` чистые.

- **файлы** (md5): актуальные — в таблице rework выше; исторические (первая сдача):
  `model.js 1622e3e8…`, `app.js daea5218…`, тест `bdf73f8b…`; `red.log f3d0e410…`
  (red первой сдачи), `live-measure.py 70f3c09e…`, `live-smoke.log 2426b803…`.

- **риски/вопросы**:
  1. ~~`index.html` не менялся / динамический import~~ — закрыто rework R2: статический
     модуль, `index.html` правится по расширенной матрице; подтверждено live smoke.
  2. `stats().pages` трактован как **всего страниц** (`ceil(totalRows/pageSize)`), а не как
     число загруженных; в ТЗ §4 поле не расшифровано — зафиксировано тестом.
  3. `ensureRange` отвергает диапазон шире `maxPages` (`RangeError`) вместо молчаливой
     частичной выдачи: видимое окно (≈34 строки при `pageSize=100`) в бюджет укладывается
     всегда, но контракт стоит подтвердить на приёмке.
  4. ~~Placeholder без CSS~~ — закрыто rework R3: `.placeholder` в `style.css`, текст
     плейсхолдера убран.
  5. Замер предварительный (мой скрипт-адаптация); финальные числа — за PM на финальном
     бинаре после приёмки.

- **следующий шаг**: PM — финальный `remeasure` на бинаре после batch, затем ревью (REV-1) и
  мутации (QA-1) по назначенным файлам.
