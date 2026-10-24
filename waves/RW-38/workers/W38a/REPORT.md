# W38a — DOM-тесты GUI-спайка на настоящем `app.js` (RW-38)

- **batch/карточка**: RW-38 / DoD §2.1 (`tests/support/gui-dom-harness.mjs`,
  `tests/unit/gui-spike-dom.test.mjs`);
- **сделано**:
  - `tests/support/gui-dom-harness.mjs` — минимальный DOM-шим без npm-зависимостей:
    - `ShimDocument`/`ShimElement` (+`HTMLInputElement`, `HTMLButtonElement` для `instanceof`
      в `app.js`), `classList`, `dataset`, `StyleDecl`, `textContent`, `append`,
      `appendChild`, `replaceChildren`, `remove`, `parentNode/children`, `title`,
      `scrollTop/clientHeight/scrollHeight`, `addEventListener/removeEventListener`,
      `dispatchEvent/emit/click` (у checkbox `click()` сначала переключает `checked`);
    - селекторы, которые реально использует `app.js`: `#id`, `.class`, `tag`, `tag.class`,
      группы `.a, .b`, compound `.screen.active`, потомок `#member-list .member`,
      атрибут `input[type="checkbox"]`; порядок выдачи — документный;
      `getAttribute/setAttribute/removeAttribute/hasAttribute` (прочие атрибуты — в Map,
      чтобы ARIA-правки W38c не ломали шим);
    - DOM повторяет `ui/index.html`: `rail` + 4 `.rail-btn[data-screen]`, `main`/`topbar`/
      `.screen-title`/`.metrics`, 4 `.screen`, `chat-list`, `emoji-head`/`emoji-search`/
      `emoji-tabs`/`emoji-grid`, `member-list`, `settings`, `bench`;
    - `ResizeObserver` (observe/unobserve/disconnect + `triggerResize()` вызывает колбэк
      только для наблюдаемых целей), `requestAnimationFrame` (очередь + `flushRaf()`,
      кадр = +16.7 мс), `performance.now`, `setTimeout`/`setInterval` (только записываются,
      `flushTimers()`), `window.__TAURI__.core.invoke` с журналом `invokeCalls`
      (`spike_config → {autobench:false}`, `process_metrics → rss/pss/threads/cpu`,
      `report_ui_snapshot`/`report_bench → null`, неизвестная команда → reject);
    - `loadApp()` выставляет глобалы, импортирует
      `ui/app.js?test=<n>` (свежий экземпляр модуля на тест, статический
      `import { PageStore }` резолвится в настоящий `ui/model.js`), прогоняет 30 кадров
      с дренажом микрозадач (авто `report_ui_snapshot` успевает опубликоваться);
    - `GUI_DOM_APP=<path>` — прогон против копии `app.js` (red-first мутации без правки
      сдаваемого файла);
    - `dispose()` снимает патч консоли, чистит таймеры/rAF/observers.
  - `tests/unit/gui-spike-dom.test.mjs` — **20 кейсов** (≥15 по DoD), реальный `app.js`:
    1. шим-разметка сверена с `ui/index.html` (все `id`, все классы, `screen-*`, `data-screen`);
    2. 4 экрана: клик по каждому `.rail-btn` делает active ровно одну кнопку и меняет
       `.screen-title`;
    3. чат: sizer = `100000*44`, в DOM десятки строк, не сотни тысяч; нижняя строка — `#99999`;
    4. чат: `top` каждой строки = `index*44`, окно накрывает запрошенный индекс (±overscan);
    5. участники: sizer = `24229*40`, ≥10 строк, строка = avatar+имя+роль;
    6. скролл участников: окно сдвигается, старые строки удалены (`parentNode=null`),
       `sizer.children === rows`, свип по 1000/5000/300 не растит DOM;
    7. скролл чата: то же для строки `#99999` → 60000;
    8. `PageStore`: `chatLoadedRows`/`memberLoadedRows` целые, кратны 100, `≤500`, `< total`;
    9. эмодзи: 10 вкладок, активна ровно одна, клик по второй меняет и вкладку, и сетку;
    10. эмодзи-поиск: фильтр → 0 ячеек, сброс → исходное число, цикл вкладок не копит
        ячейки (≤120) и не даёт дублей;
    11. настройки: 38 чекбоксов, 38 `.toggle`, 3 группы с ожидаемыми заголовками, все включены;
    12. клик по тумблеру меняет `checked` (и обратно), число тумблеров не меняется;
    13. `UI_SNAPSHOT`: `screenCount=4`, `chatRows=100000`, `memberRows=24229`,
        `emojiCategories=10`, `settingsToggles=38`, `rendered≥10`, `loadedRows≤500`,
        `actions` = `{screenClicks:4, categoryClicks:true, toggleClick:true, listScroll:true}`;
    14. `autobench=false`: `report_ui_snapshot` ровно 1, `report_bench` 0 (лишние кадры не
        запускают замер);
    15. `process_metrics`: `#m-rss` = `N МБ`, `#m-threads` = 7, интервал 2000 мс записан и
        срабатывает только по `flushTimers()`;
    16. нет дублей: в каждой строке ровно один `.author`/`.text` (placeholder без детей),
        элемент строки переиспользуется при resize без накопления детей. Переход
        `row→placeholder→row` на одном элементе через публичный DOM недостижим (страницы
        окна пиннятся `PageStore`, см. M2) — достижимая сторона того же контракта
        «чистить перед рендером» закрыта кейсом 10 (`renderEmoji`, мутация M3);
    17. граничный скролл: 0 / −50 / последняя строка / `1e9` — без исключений, за концом
        `rows=0`, sizer сохраняется, `console.error` нет;
    18. `ResizeObserver`: рост высоты контейнера расширяет окно, возврат — ужимает до
        исходного;
    19. скрытый экран рисует только overscan (<10), показ перерисовывает окно (≥20) — P1-1;
    20. обычная работа без `console.error` и предупреждений виртуализации.
- **red** (мутации в копии `ui/app.js`, сдаваемый файл не менялся; `waves/RW-38/workers/W38a/`):
  - baseline `green-baseline.log` — 20/20 pass;
  - `M1-chat-rows-500` (`CHAT_ROWS=100_000`→`500`) → **5 fail** (sizer/окно/top/скролл/snapshot);
  - `M3-no-emoji-replaceChildren` (`emojiGrid.replaceChildren(`→`append(`) → **2 fail**
    («10 вкладок», «поиск/дубли»);
  - `M4-no-resize-observe` (убрано `observer.observe(container)`) → **2 fail**
    (ResizeObserver-кейс, скрытый экран);
  - `M5-no-row-remove` (убрано `el.remove()` при выходе строки из окна) → **8 fail**
    (окно/DOM-рост/дубли/границы/скрытый экран);
  - `M2-no-row-replaceChildren` (убрано `row.replaceChildren()` в `renderChatRow`) →
    20/20 pass — **эквивалентный мутант для DOM-наблюдения**: переиспользование
    `row→placeholder→row` на одном элементе недостижимо через публичный DOM, т.к.
    `PageStore.ensureRange` пиннит страницы текущего окна и displayed-страница не
    эвиктится; placeholder→row (достижимо) детей не копит. Поэтому контракт
    «чистить перед рендером» закрыт достижимой стороной — `renderEmoji` (M3);
  - сводка: `summary.txt`, restore проверен: `app.js` md5 до и после — `32ed027bc8cf…`;
- **green**: `node --test tests/unit/gui-spike-dom.test.mjs` → `tests 20 / pass 20 / fail 0`
  (~115 мс), `green.log`;
- **targeted**:
  - `node --check tests/support/gui-dom-harness.mjs` и `...gui-spike-dom.test.mjs` — OK;
  - `npx tsc -p tsconfig.json` → exit 0 (`typecheck.log`; шим и тест под `checkJs/strict/
    noUncheckedIndexedAccess` внутри `tests/**/*.mjs`);
  - `node --test tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs
    tests/unit/gui-spike-dom.test.mjs` → 91/91 pass (модель и бенч не затронуты);
- **oracle** (независимый от реализации):
  - структура DOM — сам `ui/index.html` (id/классы/`data-screen`), а не константы в тесте;
  - `totalRows` подтверждён дважды: высотой sizer (`total*rowHeight`) и связью
    `индекс строки (из текста #N) ↔ style.top = N*44`;
  - числа контракта — `waves/RW-38/TZ.md` §4 и `tests/smoke/gui-spike.smoke.test.mjs`
    (те же пороги: 4/100000/24229/10/38, rendered≥10, loadedRows≤500, actions);
  - IPC-вызовы — журнал `invokeCalls` (как `UI_SNAPSHOT` в stdout у Rust-битаря);
  - внутренний `scrollToIndex` проверяется через `actions.listScroll` в `UI_SNAPSHOT`
    (кейс 13), пользовательский скролл — через `scrollTop` + событие `scroll` (кейсы 3–7,
    17), т.е. оба пути в `VirtualList` покрыты;
- **генерация**: не применимо;
- **мутации**: см. red (M1/M3/M4/M5 пойманы, M2 эквивалентен для DOM-наблюдения);
- **baseline/restore**: `app.js` md5 `32ed027bc8cf5c72f5a0ca4705706eab` до и после мутаций
  (мутации — в `/tmp/opencode/rw38-mut/*/ui/app.js`); `ui/*`, `model.js`, `main.rs`,
  бенч/смоук не менялись;
- **стабильность**: каждый кейс — свежий экземпляр модуля (`?test=<n>`) и свой документ;
  `setTimeout/setInterval` не тикают сами (guard-таймеры `benchScroll`/`nextFrame` не дают
  nondeterminism); 5 прогонов подряд — 20/20 pass в каждом; полный файл ~0,1 с;
- **gate:worker**: воркер полный gate не запускает (batch-вердикт — Integration/QA);
  локально — targeted-тесты, `tsc`, `node --check`, мутации;
- **файлы + md5**:
  - `tests/support/gui-dom-harness.mjs` — `134793a555359be1f6b75076d61015b4`
  - `tests/unit/gui-spike-dom.test.mjs` — `b0040aa720c6649cb6f7a23256ebd6ed`
  - `waves/RW-38/workers/W38a/` — логи: `green.log`, `typecheck.log`, `green-baseline.log`,
    `mut-M*.log`, `summary.txt`, `run-mutations.sh`;
- **риски/ограничения шима** (все — осознанные, записаны в шапке харнесса):
  - layout отсутствует: `clientHeight` — заданная харнессом `boxHeight` (800/600) или 0 для
    скрытого `.screen`; `scrollHeight` — по `top+height` детей. Реальные CSS-размеры,
    шрифты, DPI и `getBoundingClientRect` не моделируются;
  - события не всплывают, нет capture/bubbling и делегирования; `click()` вызывает только
    собственные слушатели (в `app.js` все обработчики — на самих элементах);
  - поддерживается только подмножество селекторов (потомок и compound; без `>`, `+`, `~`,
    псевдоклассов). Если W38c введёт селектор вне подмножества — харнесс вернёт пустой
    список, это ловится первым кейсом/падением теста, а не тихим skip;
  - `setTimeout/setInterval` не тикают автоматически — тест обязан звать `flushTimers()`;
  - `row→placeholder→row` на одном элементе недостижим (см. M2): соответствующий
    `replaceChildren` в `renderChatRow` защитный, проверяется только структура строк;
  - шим не эмулирует Tauri/WebKit (GBM, rAF-occlusion, AT-SPI) — это остаётся live smoke и
    `scripts/gui-window-probe.py` (W38c);
  - `GUI_DOM_APP` — тестовый хук; на боевой прогон не влияет (default — настоящий `ui/app.js`).
- **совместимость с W38c**: ARIA-правки (`setAttribute("aria-*")`, `role`,
  `element.role = …`) шим принимает; если ARIA добавит в `index.html` новые id/классы,
  кейс №1 сразу покажет расхождение шима с разметкой.
