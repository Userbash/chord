# REVIEW-197A — Техническое ревью PR-169 (`task-290` / `RE-22-04`: `voice-diagnostics.js` + `voice-diagnostics.test.mjs`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-197A
- **Проверяемая ветка:** `feature/task-290-fe-voice-diagnostics` (база `unstable`, HEAD `70c0473`)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, обязательные мутации `5/5` убиты (100%), плюс `2/2` дополнительных пробы; оба файла восстановлены, MD5 совпадают с `REPORT.md`

## 1. Объект ревью и целостность файлов

| Файл | MD5 в `REPORT.md` | MD5 до проверки | MD5 после цикла мутаций |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/features/voice-diagnostics.js` | `75a2a965b69d65471377f8c0506895ac` | совпал | `75a2a965b69d65471377f8c0506895ac` |
| `tests/unit/voice-diagnostics.test.mjs` | `4faed2e5814fee87d04df0bd9ad294cc` | совпал | `4faed2e5814fee87d04df0bd9ad294cc` (не менялся) |
| `TDD-PLAN.md` | `66f4670fb4dbb6bd9c13bd21af61a35d` | совпал | `66f4670fb4dbb6bd9c13bd21af61a35d` (не менялся) |

Перед циклом `voice-diagnostics.js` скопирован в `/tmp/opencode/tr197a/voice-diagnostics.js.orig`. Харнесс `mutations/mutate-197a.sh` после каждой мутации возвращает файл из бэкапа и сверяет MD5; при расхождении завершается кодом `3`. Финальная строка прогона: `ИТОГ: исходник 75a2a965b69d65471377f8c0506895ac (ожидался 75a2a965…)`.

Тест в цикле мутаций не редактировался — это принципиально: иначе «убийство» мутанта ничего не доказывает.

## 2. Состав изменений (`git status --porcelain`)

```
 M TDD-PLAN.md
 D node_modules
 M package-lock.json
?? rust/gui-spike/ui/features/voice-diagnostics.js
?? tests/unit/voice-diagnostics.test.mjs
```

Предмет PR-169 — два новых файла (`??`) и одна строка в §8 `TDD-PLAN.md`. `D node_modules` (симлинк `120000` развёрнут в каталог) и `M package-lock.json` (в корневую запись добавлены `bin`/`engines`, версии зависимостей не менялись) — предсуществующий шум рабочего дерева, не часть PR; см. `P3-5`.

Оба новых файла не в `.gitignore` (`git check-ignore` — код `1`), то есть попадут в коммит при `git add`. `voice.js` и `voice-quality.js` не тронуты — границы write-scope соблюдены (`TASK.md`: 3 файла).

## 3. Сверка с RE-оракулом

Проверял сам, по `reference-oracle/decoded/3eefa7dae1e2c234_0` (read-only) и `waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`, не по тексту `REPORT.md`.

| Элемент контракта | Оракул (offset в бандле) | Реализация | Итог |
| --- | --- | --- | --- |
| Пороги `quality` | `@2469483`: `e>500\|\|…>10 ? BAD : e>250\|\|…>5 ? AVERAGE : FINE` | `evaluatePingQuality` — те же `>` и те же границы | совпадает |
| Буфер пингов 199 | `@2504775`: `while(this._pings.length>=200)this._pings.shift()` | `PING_HISTORY_MAX_POINTS=199`, `slice(-199)` | совпадает |
| Средний пинг по 20 | `@2475383`: `_pings.slice(0,Math.min(len,20))` | `PING_AVERAGE_WINDOW=20` | совпадает (константа, в модуле не используется) |
| Таймаут пинга 4000 мс | `@10828400`: `i>0?i:4e3` (плюс `nK=4e3` в блоке `MediaEngine`, `@314006`) | `DEFAULT_PING_TIMEOUT_MS=4000` | совпадает |
| Спарклайн 258×80 | `@7459995`: `{dataPoints:e.pings,width:258,height:80}` | `SPARKLINE_WIDTH/HEIGHT` | совпадает |
| Короткий хост | `@9829977`: `getShortHostname: e=>null==e?"":e.split(".")[0]` | regex `/^[^.:]*/` + `trim()` | состав совпадает, есть расширение (см. `P3-1`) |
| Формат метрик попапа | `@7459662`: `badPing:250,badLossRate:10`; `@7460329`: `averagePing.toFixed(0)`, `lastPing.toFixed(0)`, `outboundLossRate?.toFixed(1)` | `formatPopoutMetricsRu`: `Math.round` + `toFixed(1)` | совпадает |
| Privacy Code 30 цифр / 6×5 | `@7462562`: `chunkSize:5,desiredLength:30` | `formatVoicePrivacyCodeChunks` + `Object.freeze` | совпадает |
| Секции `RTCDebugStore` | `@239814`: `t$=…TRANSPORT, OUTBOUND, SCREENSHARE, INBOUND, STREAMS_OUTBOUND, STREAMS_INBOUND, CAMERA, CLIPS` | `RTC_DEBUG_SECTIONS`: 8/8 тех же значений, **другой порядок** | состав совпадает, порядок расходится (см. `P2-2`) |
| Окно отладки 854×480 | в `3eefa7da…` только регистрация ленивого чанка `webpackId:767675`, `@1896599`; сами размеры — в CSS чанка | `DEBUG_MODAL_WIDTH/HEIGHT_PX` | из `RE-GAP-22`, в этом дампе не перепроверяется (риск низкий: константа не участвует в логике) |

Логика `evaluatePingQuality` — построчное соответствие геттеру `quality`: сначала `bad` (`>500`/`>10%`), затем `average` (`>250`/`>5%`), иначе `fine`. Проверено вручную на границах: `250/251`, `500/501`, `5/5.0001`, `10/10.0001` — тесты бьют ровно в те же точки.

Тесты сверяют контракт с оракулом, а не с реализацией: числа `250/500/5/10/199/20/4000/258/80` и список секций продублированы в тесте литералами, импорт констант используется только для границ и для итерации по секциям. Тавтологии («сравнить функцию с ней же») нет.

## 4. Проверки

| Проверка | Результат | Лог |
| --- | --- | --- |
| `node --test tests/unit/voice-diagnostics.test.mjs` | **67/67 pass**, 14 suites, 0 fail, 0 skip | `mutations/baseline.log` |
| `npx tsc -p tsconfig.json --noEmit` | exit `0`, 0 ошибок; оба файла в программе (`--listFiles`) | `mutations/baseline.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | exit `0`, 0 ошибок; модуль в программе (`--listFiles`) | `mutations/baseline.log` |
| `node --test tests/meta/plan-sync.test.mjs` | **5/5 pass** | `mutations/baseline.log` |
| Регрессия: 10 voice-файлов (`voice-diagnostics`, `voice-ui`, `voice_ui`, `voice-quality`, `voice-devices-settings`, `voice-grid`, `voice-volume`, `voice-stage-ui`, `voice-input-mode`, `voice-sound-effects`) | **262/262 pass**, 0 fail | прогон ревьюера |
| Полный `npm run test:unit` | **2005/2005 pass**, 390 suites, 0 fail, 0 skip, exit `0` | `mutations/full-unit.log` |

`--listFiles` важен именно затем, чтобы «0 errors» относилось к правленому коду: модуль попадает в UI-программу напрямую, тест — через `include: tests/**/*.mjs` в корневой.

`red.log` аутентичен: до реализации тест падал на `ERR_MODULE_NOT_FOUND` по импорту модуля, `pass 0 / fail 1`, `EXIT=1`. То есть red-first подтверждён фактом, а не декларацией.

## 5. Мутации

Методика: `mutations/mutate-197a.sh` — точечная замена одной конструкции (python3, проверка «ровно одно вхождение», иначе код `5`), прогон `node --test tests/unit/voice-diagnostics.test.mjs`, затем откат из бэкапа с проверкой MD5. Логи: `mutations/M1.log … M5.log`, `mutations/B1.log`, `mutations/B2.log`, полный вывод с диффами — `mutations/run.log`, сводка — `mutations/results.json`.

| # | Мутация | Итог | Убивший тест | pass/fail |
| --- | --- | --- | --- | --- |
| M1 | `evaluatePingQuality`: `pingMs > 500` → `>= 500` | **убит** | `пинг 500 мс — average, 501 мс — bad` | 66/1 |
| M2 | `formatPopoutMetricsRu`: порог предупреждения по потерям `10%` → `5%` | **убит** | `границы предупреждения: 250 мс и 10% не warn, 251 мс и 10.1% — warn` | 66/1 |
| M3 | `extractShortHostname`: `/^[^.:]*/` → `/^[^.]*/` (порт не отсекается) | **убит** | `пустой, нестроковый и «только порт» вход даёт пустую строку` | 66/1 |
| M4 | `formatVoicePrivacyCodeChunks`: `^\d{30}$` → `^\d{29,}$` | **убит** | `неверная длина (29 / 31 цифра) → null` | 66/1 |
| M5 | `computeSparklinePoints`: `slice(-199)` → `slice(-20)` | **убит** | `хранит только последние 199 точек (PING_HISTORY_MAX_POINTS)` | 66/1 |

**Итого `5/5` (100%), обязательный минимум закрыт.** После цикла `voice-diagnostics.js` = `75a2a965b69d65471377f8c0506895ac`.

Дополнительные пробы (не входят в 5/5):

- `B1` — снят guard идемпотентности в `open()`: **убит** (`open() … повторный open не дублирует загрузку`).
- `B2` — `Escape` закрывает сначала попап, потом модалку: **убит** (`Escape при открытой модалке закрывает модалку, попап остаётся открытым`).

То есть покрыты не только чистые форматтеры, но и состояние контроллера: обе ключевые инварианты (`open` идемпотентен, `Escape` разбирает стек слоёв) закреплены тестами.

## 6. Замечания

Блокирующих нет. Всё ниже — `P2`/`P3`.

### P2-1. `copyPrivacyCode()` не валидирует `privacyCodeChunks`
JSDoc модуля и `REPORT.md` §4 обещают: «копирует только валидный 30-значный код». Ветка `privacyCode` действительно проходит через `formatVoicePrivacyCodeChunks`, а ветка `privacyCodeChunks` берёт массив как есть — лишь бы он был непустым:

```js
if (Array.isArray(snapshotChunks) && snapshotChunks.length > 0) {
  chunks = Object.freeze(snapshotChunks.map(String));
}
```

Проба (`mutations/probe-p2.log`): `updateSnapshot({privacyCodeChunks:['abc','de']})` → `copyPrivacyCode() === true`, флаг «Скопировано» выставлен, в колбэк уходит мусор; `['12345','67890']` копируется как `1234567890` (10 цифр). Тест `невалидный или пустой код не копируется` закрывает только `privacyCodeChunks: []`, поэтому дефект не виден.

Почему не `P1`: модуль пока не подключён к DOM/`voice.js`, а снимок формирует BFF, который сам отдаёт чанки из `formatVoicePrivacyCodeChunks`. Практический ущерб — «скопирован неполный код» при битом снимке. **Рекомендация:** прогнать чанки через тот же оракул — `formatVoicePrivacyCodeChunks(chunks.join(''))` — и добавить тест на `['abc','de']` и на 5 чанков вместо 6. Правка на одну строку, тест на три.

### P2-2. Порядок `RTC_DEBUG_SECTIONS` не совпадает с порядком `zWA` в бандле
В бандле (`@239814`) enum объявлен как `TRANSPORT, OUTBOUND, SCREENSHARE, INBOUND, STREAMS_OUTBOUND, STREAMS_INBOUND, CAMERA, CLIPS`. В модуле и в тесте — `TRANSPORT, OUTBOUND, INBOUND, SCREENSHARE, CAMERA, CLIPS, STREAMS_OUTBOUND, STREAMS_INBOUND` (этот же порядок продублирован в `RE-GAP-22` §1). Набор из 8 значений совпадает, расходится только порядок.

Функционально сейчас это ни на что не влияет: `RTCDebugStore` использует `d.zWA` только как словарь (`d.zWA.TRANSPORT`, `@9264866`), секции не итерируются; рендер модалки в этот срез не входит. Но комментарий «в порядке `zWA`» и тест `RTC_DEBUG_SECTIONS — … в порядке zWA` утверждают факт, которого нет. **Рекомендация:** либо привести константу к порядку бандла вместе с тестом (это исправление оракула, не ослабление проверки), либо снять из комментария слова «в порядке `zWA`» и сослаться на `RE-GAP-22` §1. Делать это лучше отдельной правкой со ссылкой на offset, чтобы не перепутать с «подгонкой теста под код».

### P3-1. `extractShortHostname` режет ещё и по `:`
Оракул — буквально `split(".")[0]`, без обработки порта. Наш модуль дополнительно отсекает `:`: `'finland10192.discord.media:443'` → `finland10192` (совпадает с оракулом), но `':443'` → `''` (оракул вернул бы `':443'`). Расширение разумное и задокументировано в JSDoc; тест `пустой, нестроковый и «только порт» вход` его фиксирует. Оставляю как осознанное отклонение, менять не обязательно.

### P3-2. `evaluatePingQuality` возвращает `unknown` при потерях вне `[0..100]`
Оракул на `50 мс / 101%` дал бы `bad` (просто `> 10`), наш модуль — `unknown` (проба в `probe-p2.log`). В реальном пайплайне недостижимо: `OutboundLossRate` считается как `clamp(deltaLost/(deltaSent+deltaLost), 0, 1) * 100` (`RE-GAP-22` §2.3), то есть всегда `[0..100]`. Отклонение задокументировано в JSDoc и закреплено тестом — принимаю как защитную нормализацию. Вопрос стиля: `pingMs` строго число, а `outboundLossRatePct` приводится через `Number()`, то есть `"12"` даёт `bad`, а `"250"` в пинге — `unknown`. Асимметрия безвредна, но её стоит держать в голове при подключении к BFF.

### P3-3. `formatPopoutMetricsRu` печатает невалидные числа как есть
Проба: `{averagePingMs: Infinity}` → `Средняя задержка: Infinity мс`, `{lastPingMs: -42}` → `Последняя задержка: -42 мс`. Единственная защита — `Number(x) || 0`, которая ловит `NaN`, но не `Infinity` и не отрицательные значения. Тестов на эти входы нет. **Рекомендация:** нормализовать через `Number.isFinite(v) && v >= 0 ? Math.round(v) : 0` (или прочерк) и добавить по одному assert на `Infinity`/`-1`.

### P3-4. `updateSnapshot` копирует снимок на один уровень
`{...snapshot}` защищает верхний уровень (тест `updateSnapshot копирует снимок` это проверяет), но вложенный массив `pings` остаётся общим: после `pings.push(9999)` состояние показывает `[10,20,9999]` (проба в `probe-p2.log`). Пока снимок формирует BFF и никто не мутирует массив постфактум, риска нет. Если понадобится изоляция — `pings: snapshot.pings ? [...snapshot.pings] : snapshot.pings`.

### P3-5. Шум рабочего дерева, не входящий в PR
`D node_modules` (симлинк развёрнут в каталог), `M package-lock.json` (только `bin`/`engines` в корневой записи). Тот же шум, что в `REVIEW-194A` `P2-3` и `REVIEW-195A` `P2-2`; к task-290 отношения не имеет. **Рекомендация:** merge только трёх путей PR-169, lock и симлинк чинить отдельной инфраструктурной карточкой.

## 7. Соответствие DoD

| Критерий | Статус |
| --- | --- |
| MD5 файлов совпадают с `REPORT.md` | да, до и после мутаций |
| Правка только в разрешённых файлах (2 новых + строка в `TDD-PLAN.md`) | да; `voice.js` не тронут |
| `node --test tests/unit/voice-diagnostics.test.mjs` = 67/67 | да, 0 skip |
| `npx tsc -p tsconfig.json --noEmit` = 0 ошибок | да, оба файла в программе |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` = 0 ошибок | да, модуль в программе |
| `node --test tests/meta/plan-sync.test.mjs` = 5/5 | да |
| Red-first подтверждён | да (`red.log`: `ERR_MODULE_NOT_FOUND`, 0 pass / 1 fail) |
| Тесты сверяют контракт с независимым оракулом | да (литералы из бандла, не импорт констант) |
| 9 измерений покрытия | да, сьюты размечены по измерениям, включая загрузочные/восстановление/наблюдаемость |
| Мутации | `5/5` убиты + `2/2` пробы; файл восстановлен побайтово |
| Регрессия | 262/262 по voice-срезу, 2005/2005 полный unit |
| `git commit/push` не делался | да |
| `P0 = 0`, `P1 = 0` | да |

## 8. Артефакты

```
waves/RW-40/G2/task-290-fe-voice-diagnostics/REVIEW-197A.md          — этот отчёт
waves/RW-40/G2/task-290-fe-voice-diagnostics/mutations/mutate-197a.sh — харнесс мутаций с откатом и проверкой MD5
waves/RW-40/G2/task-290-fe-voice-diagnostics/mutations/run.log        — полный прогон: дифф + тесты + вердикт по каждому мутанту
waves/RW-40/G2/task-290-fe-voice-diagnostics/mutations/M1.log … M5.log — логи обязательных мутантов
waves/RW-40/G2/task-290-fe-voice-diagnostics/mutations/B1.log, B2.log  — дополнительные пробы (обе убиты)
waves/RW-40/G2/task-290-fe-voice-diagnostics/mutations/baseline.log   — 67/67 + tsc ×2 + plan-sync 5/5
waves/RW-40/G2/task-290-fe-voice-diagnostics/mutations/full-unit.log  — 2005/2005 (полный unit)
waves/RW-40/G2/task-290-fe-voice-diagnostics/mutations/probe-p2.mjs   — скрипт проб P2/P3
waves/RW-40/G2/task-290-fe-voice-diagnostics/mutations/probe-p2.log   — вывод проб + выдержки оракула
waves/RW-40/G2/task-290-fe-voice-diagnostics/mutations/results.json   — сводка: MD5, мутации, вердикты
```

**Вердикт: APPROVED.** `P2-1` и `P2-2` — рекомендации на отдельные правки (`P2-1` — одна строка + три assert'а, `P2-2` — либо порядок, либо комментарий); слияние в `unstable` ими не блокируется, потому что модуль ещё не подключён к DOM/BFF и оба пункта не меняют текущего поведения. Следующий статус — `Ready for Merge`; merge трёх путей и запись события в `waves/db.jsonl` выполняет координатор.
