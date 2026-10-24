# RW-37 — независимое код-ревью (REV-1, codex)

- Волна/карточка: RW-37 «оконная/постраничная модель данных (ADR-1 follow-up)».
- HEAD: `0239543`; файлы волны не закоммичены (untracked) — ожидаемо.
- Ревьюер: REV-1. Продуктовый код **не менялся**; все проверочные артефакты — в `/tmp/opencode/`.
- Метод: чтение кода/тестов/логов, независимый прогон тестов и `tsc`, собственный fuzz/race-пробник
  `PageStore`, независимый live-перезамер PSS на бинаре `12f08324…`, распаковка встроенных в бинарь
  ассетов для проверки провенанса замеров, повторный прогон `RE/extract.py` (diff = 0).

**Вердикт: PASS (P0 = 0, P1 = 0).** P2 — 5 находок (закрыть или осознанно отложить при приёмке),
P3 — 8 замечаний. Метрика PSS подтверждена независимо: **276,2 МБ** (≤300 МБ), подмены PSS→RSS нет.

---

## 1. Метрика: PSS ≤ 300 МБ достигнут, метрика не подменена

| Источник | total PSS | WebKitWebProcess | Бинарь |
|---|---|---|---|
| RW-36 baseline (`waves/RW-36/pss.json`) | 403 328 КиБ ≈ **393,9 МБ** | 288 391 КиБ | `2a47bd3c…` |
| RW-37, записанный замер (`waves/RW-37/pss.json`) | 280 797 КиБ ≈ **274,2 МБ** | 165 814 КиБ | `12f08324…` (по mtime сборки 19:37 → замер 19:39 и `app.log`; хеш в `pss.json` не пишется — P2-4) |
| **REV-1, независимый перезамер** (`/tmp/opencode/rev1-pss.json`) | 282 867 КиБ ≈ **276,2 МБ** | 165 821 КиБ | `12f08324…` |

- Метод одинаков в обеих волнах: `smaps_rollup` (`Pss:`) по дереву из 3 процессов —
  `waves/RW-36/measure-pss.py`, и это ровно та метрика, которую ADR-1 §4.2 объявил бюджетной
  («Метрика памяти — PSS дерева процессов, а не RSS родителя»). Подмены нет.
  Для справки: RSS-сумма ≈458–460 МБ — она бюджет не проходит, но по ADR-1 она и не бюджетная.
- Падение PSS объяснимо: `WebKitWebProcess` 288 391 → 165 814 КиБ (≈281,6 → 161,9 МБ, −119,7 МБ)
  при удалении 124 229 материализованных JS-объектов; main-процесс без изменений
  (82 780 → 82 798 КиБ), NetworkProcess +2 МБ (shared-страницы WebKit).
- FPS/p95: `BENCH {"chat":{"fps":60,"avgMs":16.67,"p95Ms":18,"frames":300},"members":{…}}`,
  `lowSample`/`stalled` отсутствуют — и в записанном `app.log`, и в моём прогоне.
- `UI_SNAPSHOT` воспроизводится байт-в-байт: `chatRenderedRows 30`, `memberRenderedRows 24`,
  `chatLoadedRows 400`, `memberLoadedRows 100` → 400 ≤ 100×5 и 100 ≤ 100×5; 124 229 объектов
  больше не материализуются.

### 1.1 Провенанс бинаря: замер соответствует сдаваемому `ui/*` (важно)

Сейчас `mtime` бинаря (19:37:34) **старше** `ui/app.js` (19:44) и `ui/model.js` (19:46), потому что
мутационный харнесс перезаписывает файлы и ставит им новый `mtime` (см. P2-3). Чтобы не полагаться
на mtime, я проверил **содержимое**, встроенное в бинарь: `tauri::generate_context!` держит ассеты
в `target/debug/build/discord-gui-spike-ccbb4ccf9d5ca332/out/tauri-codegen-assets/` (brotli).
Распаковка и сравнение:

```
ui/app.js    23860 == 23860  IDENTICAL
ui/model.js   8769 ==  8769  IDENTICAL
ui/index.html 2029 ==  2029  IDENTICAL
ui/style.css  4163 ==  4163  IDENTICAL
```

То есть замер `274,2 МБ` сделан на **том же контенте**, который лежит в сдаваемых `ui/*`; подмены
содержимого нет. Дополнительно: dep-info крейта (`target/debug/deps/discord_gui_spike-*.d`) содержит
`ui/app.js, ui/model.js, ui/index.html, ui/style.css` — значит `cargo build` после правок `ui/*`
пересоберёт бинарь (и sha256 изменится → см. P2-4).

---

## 2. Контракт `PageStore` (TZ §4) — инварианты держатся

Проверено тестами контракта и независимым пробником `/tmp/opencode/rev1-probe.mjs` (импорт реального
`model.js`); в колонке — основание по каждому пункту:

| Инвариант | Результат |
|---|---|
| `loadedRows ≤ pageSize*maxPages` | OK: 400 случайных `ensureRange` (диапазоны шире бюджета → отказ), `worst overshoot = 0`, финал `loadedPages=4/4`, `loadedRows=28 ≤ 28` (пробник) |
| `loadedPages ≤ maxPages` | OK в fuzz и в m07 |
| LRU не эвиктит текущий диапазон | OK в пределах одного `ensureRange` (тесты «ровно на бюджете» + m07); межвызовная гонка — см. P3-1 |
| `inFlight` без двойных загрузок | OK: `Promise.all([ensureRange(0,29) ×2, ensureRange(5,25)])` → ровно 3 `fetchPage` на 3 страницы (тест + пробник) |
| Идемпотентность | OK: 3 повторных `ensureRange(0,99)` → 1 `fetchPage` (тест) |
| Границы | OK: клампы `[-50,10]`, `[240,10000]`, no-op для диапазона вне данных, `first>last` (тест) |
| `getRow` вне `[0,totalRows)` | OK, без throw на `NaN/±Infinity/1.5/"0"/null/undefined/{}/[]/true` (тест + пробник) |
| Ошибки не кэшируются | OK: reject → `inFlight` очищен, повтор грузит (`attempt=2`), битая страница не в кэше (тест + пробник) |
| `TypeError` в конструкторе | OK: `totalRows/pageSize/maxPages` (0, −, дробные, NaN, Infinity, строки, null, undefined), не-объект опций, не-функция `fetchPage` (тест) |
| `TypeError` на не-массив из `fetchPage` | OK (тест) |

Отдельно проверил гонки:

- **конкурентные `ensureRange` разных диапазонов при полном бюджете** — порядок резолва не влияет
  на бюджет, но влияет на то, кто останется: если «старый» диапазон A резолвится позже «нового» B,
  `evict(pinnedA)` выбрасывает страницу B уже **после** успешного резолва `ensureRange(B)`
  (пробник: `getRow(0)=undefined`, `loadedPages=2`). В приложении это самоизлечивается за 1–2 кадра
  (`schedule → paint → повторный ensureRange`), постоянной дыры нет. Контракт стоит уточнить (P3-1).
- **`loadPage → evict`** — `evict` вызывается синхронно внутри `.then` до `finally`, транзиентного
  превышения бюджета наружу не видно; `inFlight` удаляется всегда.
- **ошибка `fetchPage`** — промис отклоняется, `Promise.all` пробрасывает, повтор возможен; ошибки
  `ensureRange` в `app.js` логируются (`console.error`), кадр не блокируется.

---

## 3. Интеграция `app.js` — окно работает, полной материализации нет

- `makeMessages(100000)`/`makeMembers(24229)` отсутствуют во всём репозитории (`grep` = 0); вместо
  них `makeMessage(i)`/`makeMember(i)` + `makePage(...)` только для запрошенной страницы.
- `PageStore` подключён статическим `import { PageStore } from "./model.js";` (app.js:7),
  `index.html` — `<script type="module" src="app.js">` (index.html:51); `modelModule`/динамического
  `import()` нет.
- `VirtualList` работает от `RowSource` (`totalRows/getRow/ensureRange`), placeholder не блокирует
  скролл; `requestWindow` перерисовывается только если данные реально пришли, иначе одно
  предупреждение на диапазон — вечного rAF-цикла нет (проверено логикой и живым прогоном: 60 fps,
  `frames=300`).
- `UI_SNAPSHOT` не потерял обязательные поля: Rust-валидатор (`SNAPSHOT_FLOORS`, 7 полей) принял
  снимок; `smoke` дополнительно проверяет `actions`, `chatRows/memberRows/emojiCategories/settingsToggles`.
  Новые поля `chatLoadedRows/memberLoadedRows` — аддитивные.
- `style.css:.placeholder` есть, текст в placeholder не пишется; `.row`/`.member` сохранены, поэтому
  `memberRenderedRows` по-прежнему ≥10 (но теперь считает и placeholder — см. P3-6).
- XSS-векторов нет: `innerHTML/eval/new Function` не используются, весь контент через `textContent`.

---

## 4. Тесты и мутации

- `node --test tests/unit/gui-spike-model.test.mjs` → **24/24 pass** (4 сьюта). Ровно те 24 теста,
  что заявлены; тесты используют только публичный API (`getRow/ensureRange/stats`) и журнал
  `fetchPage` — внутренние `pages/clock/inFlight` не читаются (grep: только `stats().pages`).
- Targeted 3 файла (`model+bench+dom`) → **64/64 pass**; `tsc -p rust/gui-spike/ui` и root → **0**.
- Red-first реален: `red.log` (ERR_MODULE_NOT_FOUND — тесты до кода), `red-r1.log` (22 pass/1 fail —
  новый тест короткой страницы), `red-m07.log` (1 fail на мутанте `evict` без `pinned`).
- **m07 перепроверен независимо**: скопировал `model.js`+тест в `/tmp`, убрал `pinned.has(...)`,
  прогнал `--test-name-pattern="m07"` → падает на `getRow(0)` (`actual: undefined`,
  `expected: {id:0}`) — мутант убит именно этим тестом. «Зеленения» нет.
- Мутационный лог: caught 11, survived 4, compile-error 1; m01/m10 — корректно разобраны как
  эквивалентные (`index === totalRows` всегда даёт `undefined`; `clock` строго возрастает); a02 —
  разобран как пробел интеграции с планом проверки (см. P3-4).
- Тесты не подгонялись под реализацию: единственная зафиксированная трактовка — `stats().pages =
  ceil(totalRows/pageSize)` (в TZ §4 поле не расшифровано, см. P3-3).

---

## 5. RE-контракт (W37a)

- `RE/RESEARCH.md` + `RE/layout.json` существуют; каждое число в RESEARCH имеет колонку
  «Источник/Как извлечено» и статус `measured / css-derived / source / assumption`; спорное — в §8
  (A1–A10), включая `limit>100`, op14/op8 и причину sidebar=375.
- `layout.json` — валидный JSON, ключевые числа совпадают с `extract.log` (viewport 2048×1106@1.25,
  rail 64, tree 311/203, chat 1417, members 256×1025, header 49, composer 56, строки 26/48/26/42/28,
  `limit` 12/24/30/100, окно участников 100, READY 7/272/132/146).
- Воспроизводимость: `python3 waves/RW-37/RE/extract.py` → exit 0, diff с зафиксированным
  `extract.log` — **0 строк** (детерминирован).
- Golden `/home/sanya/re-deep` не изменён: файлов новее `2026-10-23 19:00` нет (дамп от oct 19),
  сеть не использовалась.
- Рекомендация RE (`pageSize=100, maxPages=2`, 200 строк) расходится с реализацией
  (`maxPages=5`, 500 строк) — не дефект (бюджет соблюдён), но расхождение стоит зафиксировать (P3-7).

---

## 6. Находки

### P0 — нет

### P1 — нет

### P2 (закрыть или осознанно отложить до приёмки)

**P2-1. placeholder не очищает старые дочерние узлы → устаревший/дублированный текст строки**
`rust/gui-spike/ui/app.js:352-358` (`renderChatRow`) и `:373-379` (`renderMemberRow`).
`row.className = "row"` сбрасывает только классы; в ветке `item === null` (`classList.add("placeholder")`)
дети не удаляются. Если видимая страница эвиктится (гонка из §2/P3-1), строка остаётся с прежним
текстом под opacity .45, а после повторной загрузки `render` **дописывает** вторые `author/text`
(append без очистки) → дубликат сообщения/имени до переиспользования элемента. Это расходится с
заявлением R3 «placeholder — только скелетон».
*Исправление:* в начале обоих рендереров `row.replaceChildren()` (или `row.textContent = ""` в
placeholder-ветке); по возможности — unit-тест с DOM-заглушкой на переход `row → placeholder → row`.

**P2-2. Пустая последняя страница принимается и навсегда считается загруженной**
`rust/gui-spike/ui/model.js:147-163`. Проверка `rows.length !== this.pageSize` есть только для
не-последней страницы; для последней допускается `0` строк. Пробник: `fetchPage(1) → []` при
`totalRows=150` → `loadedPages=1`, `getRow(100)=undefined`, повторный `ensureRange(100,149)` **не
перезагружает** (`fetchPage` вызван 1 раз) — постоянная дыра, ровно тот класс отказа, который R1
признал недопустимым для не-последней страницы. С текущим `makePage` недостижимо (всегда
`end - start ≥ 1`), но это будущий Rust-бэкенд.
*Исправление:* требовать точную вместимость для всех страниц:
`if (rows.length !== capacity) throw RangeError(...)` (сохранив текст ошибки с индексом/ожиданием/фактом),
red-first тест «пустая последняя страница → RangeError».

**P2-3. Мутационный харнесс «состаривает» `ui/*` и делает rebuild-guard красным**
`waves/RW-37/run-mutations-model.py:498-501` (`os.utime(path, (now, now))` после restore) +
`tests/smoke/gui-spike.smoke.test.mjs:22-26`.
Сейчас `GUI_SPIKE_E2E=1 node --test tests/smoke/gui-spike.smoke.test.mjs` **падает** на guard'е
(«бинарь старше ui/*»), хотя содержимое `ui/*` байт-в-байт совпадает со встроенным в бинарь
(см. §1.1) — ложное «старение» из-за mtime. Пока это не исправлено, заявленный «live smoke 1/1»
не относится к текущей версии smoke-теста (файл изменён в 19:46, позже последнего зелёного прогона),
и приёмочный прогон упрётся в guard.
*Исправление (одно из):* в харнессе сохранять `st_mtime_ns` до мутации и восстанавливать в `finally`
(`os.utime(path, ns=(atime_ns, mtime_ns))`); либо после мутаций пересобрать бинарь и пере-снять
smoke+замер; в отчёте зафиксировать, что `mtime` guard'а — прокси, а контент идентичен.

**P2-4. Нет `waves/RW-37/METRICS.md`; sha256 финального бинаря нигде не зафиксирован**
Чек-лист D1 требует «финальный бинарь пересобран; sha256 в METRICS.md». `waves/RW-37/METRICS.md`
отсутствует; `12f08324…` не встречается ни в одном артефакте волны (`grep` — только RUNBOOK/чек-лист).
Дополнительно: `pss.json` не содержит ни sha256 бинаря, ни времени старта — провенанс замера держится
только на mtime файлов; а любой `cargo build`/`cargo test` в спайке теперь пересоберёт бинарь
(dep-info включает `ui/*`) и сменит sha256.
*Исправление:* после мутаций пересобрать бинарь, прогнать `remeasure.py`, записать в `METRICS.md`
sha256 + `total_pss_kb` + FPS/p95 + ссылку на это ревью; в `remeasure.py` — печатать/писать sha256
бинарника и `started_at`.

**P2-5. Ключевой инвариант DoD не проверяется на интеграционном уровне**
`tests/smoke/gui-spike.smoke.test.mjs:51-67` и `tests/unit/gui-spike-dom.test.mjs:24-32`.
`chatLoadedRows/memberLoadedRows` уже публикуются, но smoke их не проверяет; Rust `SNAPSHOT_FLOORS`
ограничивается нижними порогами. Регрессия «снова материализуем массив, но оставим `PageStore`»
даст зелёные model/dom/smoke и будет поймана только ручным PSS-замером. Аналогично нет запрета на
`makeMessages(100000)` в dom-тесте.
*Исправление:* в smoke — `assert.ok(snapshot.chatLoadedRows <= 500)` и `<= 500` для участников
(плюс, по желанию, `chatLoadedRows < chatRows`); в dom-тест — `assert.doesNotMatch(app, /makeMessages\(\s*100_?000/)`
или проверку наличия `new PageStore(`.

### P3 (замечания, не блокируют)

1. **Межвызовная гарантия «текущий диапазон не эвиктится».** `rust/gui-spike/ui/model.js:90-132`.
   Гарантия держится в пределах одного `ensureRange`; при конкурентных разных диапазонах при полном
   бюджете позже резолвнувшийся вызов может выбросить страницу уже завершившегося диапазона
   (пробник в §2). В приложении самоизлечивается, но контракт стоит уточнить в TZ §4 или ввести
   generation-token/очередь «побеждает последний запрос».
2. **`RangeError` для диапазона шире `maxPages` не входит в замороженный TZ §4.**
   `model.js:104-108` vs `waves/RW-37/TZ.md:43-58`; воркер сам просил подтвердить (риск №3).
   Решение приемлемое, но TZ §4 (владелец PM) стоит дополнить явно.
3. **`stats().pages` не расшифрован в TZ §4** (тест фиксирует «всего страниц»). Дописать в §4.
4. **Мутационные доказательства неполные.** `mutations-model.log` по-прежнему печатает m07 SURVIVED
   (лог снят до добавления теста); baselines в логе — «23 теста», в файле сейчас 24; a02
   (`initRowLists` не вызывается) не проверен на пересобранном бинаре, хотя план в логе это требует.
   *Исправление:* прогнать `run-mutations-model.py --filter m07` (получить KILLED в логе) и a02 с
   rebuild; либо зафиксировать как осознанно отложенное. Файл `tests/smoke/gui-spike.smoke.test.mjs`
   (правка guard'а, 19:46) вообще не имеет владельца в матрице TZ §3 и md5 в отчётах.
5. **m07 зависит от реальных таймеров** (10/60 мс, `tests/unit/gui-spike-model.test.mjs:308-329`).
   На загруженном CI порядок резолва может «поплыть» → флейк. Лучше deferred-промисы с ручным
   `resolve()` вместо `setTimeout`.
6. **`memberRenderedRows` считает placeholder-строки** (`app.js:619` — селектор `.member`), поэтому
   порог `≥10` больше не доказывает, что данные участников отрисованы; компенсируется
   `memberLoadedRows`, но smoke его не проверяет (см. P2-5).
7. **RE-расхождения:** `RESEARCH.md:156` — «GUI-спайк RW-36 держит 100 000 строк в `PageStore`»
   (PageStore появился в RW-37, в RW-36 были материализованные массивы); рекомендация RE
   `maxPages=2` vs реализация `5` — зафиксировать причину (запас на прыжки скролла) или выровнять
   в P3-01. `layout.json.provenance` покрывает не все скаляры (`message_avatar`, `member_avatar`,
   `line_height`, `topbar_h`, `composer_w`, `member_list_h`) — в RESEARCH они имеют источник, но
   машиночитаемый контракт лучше дополнить.
8. **Провенанс метрики и мелочи процесса.** `measure-pss.py` (RW-36, общий) при недоступном
   `smaps_rollup` пишет процесс с `pss=0` и не считает `skipped` (в этом прогоне все 3 процесса
   ненулевые — влияние нулевое, но в долгую стоит добавить счётчик); `REPORT.md` содержит два разных
   md5 для `tests/unit/gui-spike-model.test.mjs` (`1fdcbdad…` в rework-таблице и `0b68e39e…` в m07 —
   актуален второй); `waves/RW-37/__pycache__` — мусор в артефактах волны.

---

## 7. DoD RW-37 → статус

| # | DoD (TZ §2) | Статус | Основание |
|---|---|---|---|
| 1 | `PageStore` в `model.js` по контракту §4 | **PASS** | 24 теста + независимый fuzz/race-пробник; инвариант бюджета держится; оговорки P2-2/P3-1/P3-2 |
| 2 | Тесты red-first, ≥10 | **PASS** | 24 теста; `red.log`, `red-r1.log`, `red-m07.log`; kill m07 воспроизведён независимо |
| 3 | `app.js` на `PageStore`, rendered ≥10, `UI_SNAPSHOT`/`BENCH` живы | **PASS** | `makeMessages/makeMembers` = 0 вхождений; live: 30/24 rendered, 400/100 loaded, BENCH 60 fps; обязательные поля на месте |
| 4 | PSS ≤300 МБ, 60 fps, p95 ≤20 мс, артефакты | **PASS (метрика) / PENDING (артефакт)** | 274,2 МБ записано и 276,2 МБ воспроизведено; 60 fps/p95 18; `app.log/metrics*.json/pss.json` есть. Нет `METRICS.md` и sha256 → P2-4 |
| 5 | RE: `RESEARCH.md` + `layout.json` с провенансом | **PASS** | каждое число с источником/статусом, assumption A1–A10; `extract.py` детерминирован (diff 0); golden не изменён |
| 6 | Ревью P0/P1 = 0; мутации по назначенным файлам | **PASS с P3** | P0/P1 = 0; 16 мутаций (14 `model.js` + 2 `app.js`) + импорт-проба, survivors разобраны; m07 закрыт тестом; a02 и обновление лога — P3-4 |

---

## 8. Что сделать перед приёмкой (короткий список)

1. `app.js`: `replaceChildren()` в placeholder/рендере (P2-1).
2. `model.js`: точная вместимость для последней страницы + red-first тест (P2-2).
3. Харнесс: восстановить `mtime` в `finally` **или** пересобрать бинарь и пере-снять smoke/замер (P2-3).
4. Пересобрать бинарь после мутаций, прогнать `remeasure.py`, создать `waves/RW-37/METRICS.md`
   с sha256 + PSS/FPS/p95 + датой (P2-4). **Внимание:** `cargo build`/`cargo test` в спайке
   пересоберёт бинарь (dep-info включает `ui/*`) — после этого старый `12f08324…` недействителен,
   нужен новый замер либо явная фиксация «контент идентичен, PSS перепроверен».
5. Smoke: верхняя граница `chatLoadedRows/memberLoadedRows ≤ 500` (P2-5).
6. (P3, по желанию) уточнить TZ §4 (RangeError, `stats().pages`, межвызовная LRU-гарантия), прогнать
   `--filter m07` и a02 с rebuild, заменить таймеры m07 на deferred-промисы.

---

### Воспроизведение проверок ревьюера

```bash
node --test tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs \
  tests/unit/gui-spike-dom.test.mjs        # 64/64 pass
npx tsc -p rust/gui-spike/ui/tsconfig.json # 0
npx tsc -p tsconfig.json                   # 0
node /tmp/opencode/rev1-probe.mjs          # ALL PROBES OK (инварианты, гонки, fuzz)
python3 /tmp/opencode/rev1-remeasure.py    # PSS 282 867 КиБ = 276.2 МБ, 60 fps, p95 18
python3 waves/RW-37/RE/extract.py > /tmp/opencode/re-check.log 2>&1 \
  && diff <(sed 's/JSON-копия.*//' waves/RW-37/RE/extract.log) \
            <(sed 's/JSON-копия.*//' /tmp/opencode/re-check.log)   # diff 0
GUI_SPIKE_E2E=1 node --test tests/smoke/gui-spike.smoke.test.mjs
  # сейчас FAIL на guard'е «бинарь старше ui/*» (mtime после мутаций) → P2-3
```

Продуктовые файлы, `waves/RW-36/*` и golden не изменялись; все артефакты проверок — в `/tmp/opencode/`.
