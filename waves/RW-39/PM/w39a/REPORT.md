# W39a — отчёт: RE-артефакты Phase 1a RW-39

Карточка: RW-39 / Phase 1a (RE) — `REFERENCES.md`, `tokens.json`, `layout.json`, `README.md`.
Воркер: W39a (RE/REA). PM: DeepSeek. Итерация 1. Дата: 2026-10-23.
Статус: **выполнено, на приёмку PM**. Файлы W39b (`SPEC.md`, `tokens.css`, `DOM-CONTRACT.md`)
не изменялись; расхождения ниже — списком «на возврат PM».

## 1. Сделано

| Артефакт | Содержание |
|---|---|
| `DESIGN/REFERENCES.md` | 18 пиксельных референсов, 7 DOM-выгрузок, 3 токенных, 1 RE, 4 компонентных контракта, 2 фикстуры; у каждой строки метод и код `[M]/[T]/[L]/[A]/[F]`; §7 «чего нет в дампе» (12 пробелов); §8 юр. рамка |
| `DESIGN/tokens.json` | schema `rw39/tokens-v1`, **196 токенов** (13 alias) + 6 оверрайдов, 1:1 с `tokens.css`; у каждого: `value`, `group`, `status[]`, `dump_refs[]` (значение дампа + нормализация + match) |
| `DESIGN/layout.json` | schema `rw39/layout-v1`, **54 блока с `source`**, shell (rail/tree/chat/members × rows) + 4 экрана (`chat`, `emoji`, `members`, `settings`) + `rows{}` + breakpoints |
| `DESIGN/README.md` | команды воспроизведения, ожидаемый вывод, таблица методов и точности, юр. рамка |
| `PM/w39a/measure.py`, `measure.json`, `check.log` | воспроизводимые пиксельные замеры (PIL), ничего не пишут в golden |
| `PM/w39a/gen-tokens.mjs`, `verify-tokens.mjs`, `verify-layout.mjs` | генерация/проверка tokens.json и layout.json |

REA не применял: рантайм/бинарь не требовался, PNG/JPEG читал PIL, JSON — Node/`JSON.parse`,
провенанс зафиксирован методом в каждом блоке. Evidence REA не создавалось (нечего
открывать через `open_binary`).

## 2. Что измерено и чем

| # | Что | Значение | Метод |
|---|---|---|---|
| 1 | titlebar | **32** CSS | PNG, столбец x=1500: полоса `#121214` 0…40 px |
| 2 | шапка чата | **48** + 1px граница = **49** | PNG, полоса `#1a1a1e` 41…100 px; токен `--custom-channel-header-height=49px` |
| 3 | дерево каналов | **311.2** (граница панелей x=375.2 − rail 64) | PNG, строка y=900 |
| 4 | чат | **1416.8** | `ui8/22-members.json` x0=1792 − 375.2 |
| 5 | панель участников | **256×1025** при (1792, 82) | DOM rect `22-members.json` |
| 6 | composer | **56** высота, **1401.6** ширина (x 382.4…1784) | PNG: бокс `(34,35,39)`, столбцы ≥80 % высоты |
| 7 | user bar | **56** (y 1044…1100) | PNG, столбец x=400: `#202024` |
| 8 | строка канала | **28** высота, **30** шаг строк | PNG: выделение `#2c2c30` 28 CSS; медиана шага текстовых полос 30.0 (n=19) |
| 9 | строка участника | **43.2 ≈ 42** (допуск ±1, JPEG) | `ui8/22-members.jpg`: медиана шагов верхушек аватаров (n=16) |
| 10 | цвета поверхностей | `#121214`, `#1a1a1e`, `#202024`, `#222327`, `#2c2c30`, акцент `#5865f2` | точечные пробы lossless PNG |
| 11 | статус-точки (панель) | красная **`#da3e44`**, зелёная **`#45a366`** | частотный скан насыщенных пикселей панели |
| 12 | попаут профиля | **268.8×408** при (22.4, 608.8), фон `#2c2d32` | PNG bbox по цвету `(44,45,50)` |
| 13 | поиск в шапке | **244×28** (внутр. 242×26) | `ui8/20a-searchbar.json` |
| 14 | фильтры/сортировка | 7 секций, 5 input, 4 кнопки, 9 опций; сортировка 3 пункта | `20b-search-filters*.json`, `20c-search-sort.json` |
| 15 | эмодзи | 42 ячейки, 7 nav, 3 вкладки; **10 категорий** | `21b-emoji-full.json`, `21d-emoji-categories.json` |
| 16 | настройки | модалка **1398×961** (325,73); контент **1020×865** (633,121); **38** switch; 63 строки nav | `23-settings-categories.json`, `23-settings-voice.json` |

Сводка геометрии для приёмки: rail **64** (диапазон 64–72), tree **240** / 311 (≥1600),
header **49**, composer **56**, members **240** / 256 (≥1600), строки **26/48** (+22 line),
участник **42** (панель) / **40** (виртуализация, `[F]`). Сумма колонок референса
64+311+1416.8+256 = **2047.8 ≈ 2048** ✓.

## 3. Проверка (команды и результат)

Все команды выполнены из корня; полный лог — `waves/RW-39/PM/w39a/check.log`.

```bash
node -e "for (const f of ['tokens.json','layout.json']) JSON.parse(require('fs').readFileSync('waves/RW-39/DESIGN/'+f));"
# ok: оба JSON парсятся

node waves/RW-39/PM/w39a/gen-tokens.mjs --check
# ok: tokens.json актуален (196 токенов, md5 css 579a878aef43f8fa50a924eb4277acd9)

node waves/RW-39/PM/w39a/verify-tokens.mjs
# ok: 196 токенов 1:1 с tokens.css; dump_refs: 41 match, 16 mismatch, 1 missing

node waves/RW-39/PM/w39a/verify-layout.mjs
# ok: 54 блока с source, 22 ссылки на токены, 4 экрана; rail 64, tree 240/311,
# header 49, composer 56, members 240/256, rows 26/48 + member 42; колонки 2047.8 ≈ 2048

python3 waves/RW-39/PM/w39a/measure.py --json   # measure.json
rg -n "rail|tree|header|composer|members" waves/RW-39/DESIGN/layout.json | head
```

## 4. Расхождения с SPEC / tokens.css (решает PM, файлы W39b не правились)

Сверка `[T]`-цитат с дампом `ui/tokens.json`: 57 ссылок, 41 совпала, 16 расхождений,
1 имя отсутствует. Классификация:

### 4.1 Существенные (визуально заметно, стоит поправить в `tokens.css`/SPEC)

| Токен | В `tokens.css` | В дампе | Δ (max канал) | Комментарий |
|---|---|---|---|---|
| `--danger`, `--status-dnd` | `#ed4245` | `--status-danger` = `#da3e44` | 19 | `#ed4245` в дампе 1.0.162 **отсутствует**. `[M]` подтверждает дамп: красная статус-точка в панели = ровно `#da3e44` |
| `--warning`, `--status-idle` | `#f0b232` | `--status-warning`/`--yellow-300` = `#fdb833` | 13 | `#f0b232` в дампе нет |
| `--positive`, `--status-online` | `#23a55a` | `--status-positive`/`--green-360` = `#3d9e60` | 26 | `#23a55a` в дампе нет; `[M]` зелёная точка = `#45a366` (= `--green-new-38`). Три разных зелёных: css / status-positive / фактический |
| `--text-inverse` | `#1a1a1e` | `--text-invert` = `#2f3035` | 23 | контраст на `--warning` (SPEC §3.4: 9.91) надо пересчитать, если менять |

Вывод: в SPEC §3.3 значения `--danger/--warning/--positive` смешаны из старой палитры,
но помечены `[T]`. Решение (сменить значения на дамп или понизить код до `[A]`) — за PM/W39b.
До решения `tokens.json` честно фиксирует расхождение (`dump_refs[].match=false`), ничего
не подменяя.

### 4.2 Минорные

| Токен | `tokens.css` | Дамп | Δ | Комментарий |
|---|---|---|---|---|
| `--accent-hover` | `#4752c4` | `--brand-560` = `#4654c0` | 4 | похоже на опечатку при переносе |
| `--border-subtle/--border/--border-strong`, `--badge-secondary-bg` | база `#8e929d` | `#97979f` (альфы совпадают: 12/20/44/16 %) | 9 | базовый цвет чуть светлее |
| `--accent-soft` | `rgb(88 101 242 / 14%)` | `--message-highlight-background-default` 16 % | 2 п.п. | в комментарии стоит «≈», но SPEC §3.3 приводит 16 % как источник |

### 4.3 Эквивалентно / не ошибка

- `--radius-round`: `9999px` vs `2147483647px` — одинаково после клампа. SPEC §5.3
  утверждает «`= 2147483647px`» — формально не совпадает с CSS, менять не обязательно.

### 4.4 Несоответствие имени токена

- `--status-streaming` (`#593695`) помечен `[T] --status-streaming`, но токена с таким
  именем в дампе **нет** (есть только глифы `--icon-status-*`). У `--status-idle`,
  `--status-dnd`, `--status-offline` одноимённых токенов тоже нет — у первых двух в
  комментарии стоят `[T]`, что не подтверждается.

### 4.5 Геометрия

- Шаг строки канала: **30 CSS** (28 строка + 2 зазор) по пикселям, а RW-37 фиксирует
  только `row_heights.channel = 28`. В `tokens.css` токена зазора нет. Предложение PM:
  добавить `--row-channel-gap: 2px` или явно записать «pitch 30» в SPEC §5.1. В
  `layout.json` записаны оба числа (`h: 28`, `pitch: 30`) с источником.
- Шаг строки участника по JPEG **43.2** (n=16, ±1) против `[L] 42` — в пределах допуска,
  ничего не менял.
- Ширина рейла пиксельно не проверяема (рейл и дерево одного цвета `#121214`): 64
  подтверждается rect'ом кнопки гильдии (`w=64`) и токеном `--custom-guild-list-width`.

## 5. Пробелы дампа (детали — `REFERENCES.md` §7)

Меню гильдий (`items: 0`, html 41 байт), список ЛС (`[]`), данные результатов поиска
(`04b-search-results.json` = `{}`), текст профиля (`{"text": null}`), остальные разделы
настроек кроме «Голоса», гифки/стикеры, hover/focus-состояния, тултипы, анимации,
шрифты/иконки/спрайты (последние — намеренно, юридически). Поэтому часть чисел в
`layout.json` имеет код `[A]` с обоснованием, а не `[M]`.

## 6. Файлы + md5

| Файл | Байт | md5 |
|---|---|---|
| `waves/RW-39/DESIGN/tokens.json` | 50306 | `3041ea8c09d994e54e95a41472c74e61` |
| `waves/RW-39/DESIGN/layout.json` | 25466 | `96db9bc1bff5594b7bc5b0087f545be9` |
| `waves/RW-39/DESIGN/REFERENCES.md` | 16181 | `6d1c8d6a36373e67e31a77066102c144` |
| `waves/RW-39/DESIGN/README.md` | 7862 | `b1a2e97e077f841006ac97709dcc81bd` |
| `waves/RW-39/PM/w39a/gen-tokens.mjs` | 12707 | `1679146a3b6539e0075c8bbb24d9d8aa` |
| `waves/RW-39/PM/w39a/verify-tokens.mjs` | 5268 | `f4097fad07e6ffa779b8abea6f067cce` |
| `waves/RW-39/PM/w39a/verify-layout.mjs` | 6915 | `269d67375e27b2f3c2ee4f52e8896f89` |
| `waves/RW-39/PM/w39a/measure.py` | 9868 | `afb3698c7d6641a9e59468668de973ed` |
| `waves/RW-39/PM/w39a/measure.json` | 3668 | `dadaa48844f42a38da4942a06aa4fc39` |
| `waves/RW-39/PM/w39a/check.log` | 1507 | `ffb9d2898040231bbada053de66ee6a4` |

`tokens.css` (не менялся): md5 `579a878aef43f8fa50a924eb4277acd9` — совпадает с записью
в `tokens.json.source_css.md5` и проверяется `verify-tokens.mjs`.

## 7. Риски

1. **Палитра ≠ дамп** (п. 4.1): если W39c копирует `tokens.css` как есть, красный/зелёный/
   жёлтый в UI будут отличаться от референса на Δ 13–26 по каналу. Решение нужно до вёрстки.
2. `tokens.json` — производный файл: любая правка `tokens.css` (владелец W39b) требует
   `node .../gen-tokens.mjs` + `--check` в гейте, иначе артефакт устареет.
3. `measure.py` требует Pillow и доступ к `/home/sanya/re-deep`; при переносе golden скрипт
   упадёт явно (пути не зашиты молча) — замеры придётся повторить.
4. Шаг строки участника измерен по JPEG: ±1 CSS. Для вёрстки используется `[L]` 42 и
   `[F]` 40 — расхождение с пикселями в допуске.
5. `layout.json` не заменяет `DOM-CONTRACT.md`: геометрия не описывает структуру; W39c
   обязан брать разметку из контракта, числа — отсюда/`tokens.css`.

## 8. Границы соблюдены

- Изменены только `waves/RW-39/DESIGN/{REFERENCES.md,tokens.json,layout.json,README.md}` и
  `waves/RW-39/PM/w39a/*` (скрипты, логи, этот отчёт).
- `SPEC.md`, `tokens.css`, `DOM-CONTRACT.md` не тронуты (md5 `tokens.css` не изменился).
- PNG/JPG/шрифты/иконки/спрайты в репозиторий не копировались; `/home/sanya/re-deep` — read-only.
- Сеть не использовалась, пакеты не ставились, `git commit/push` не выполнялись.

---

## 9. Итерация 2 (2026-10-24, после рестарта сессии)

Контекст: Phase 1a принят (D-01), затем D-02/D-04 изменили `tokens.css` (196 → 204 → **205** токенов после добавления
`--row-*-pitch`/`--row-h-*`, md5 на конец итерации `7ce9c1f03e26056243e13df3237b0a9d`) и высоты строк; `tokens.json` перегенерирован
владельцем. W39a догонял артефакты до текущего состояния.

Сделано:

1. **Регрессия исправлена:** в `layout.json → discrepancies[]` ключ был переименован в `key`,
   из-за чего актуальный `verify-layout.mjs` падал (`discrepancy: нет id`, 7 ошибок).
   Восстановлен `id`; записи аудита снова проходят проверку (`severity`/`evidence`/`action`).
2. `README.md`: актуализированы числа (204 токена, 3 блока оверрайдов / 6 деклараций,
   md5 `62fffb9c…`) и ожидаемые строки верификаторов; добавлен §5 — индекс RE-деталей
   (`regions`, `screens.*.measured`, `header_spans_full_width`, `discrepancies[]`) с точками
   замера; §1 п.4 уточнён (layout не меняет контракт, а детализирует замер).
3. `REFERENCES.md` §3: вместо «41 match / 16 mismatch / 1 missing» — текущее состояние
   **57 match / 0 mismatch / 0 missing** (204 токена) + история итерации 1 и ссылка на D-02.
4. `layout.json`: сохранены `regions` (rail/tree/chat/members/userbar), `screens.*.measured`
   (эмодзи/участники/настройки), `header_spans_full_width`, `cross_check_rw37`, `assumptions`
   и 7 записей `discrepancies` с замерами.

Проверки (локально, без сети):

```
node waves/RW-39/PM/w39a/verify-tokens.mjs
ok: 205 токенов 1:1 с tokens.css (md5 7ce9c1f03e26056243e13df3237b0a9d); dump_refs: 57 match, 0 mismatch, 0 missing
node waves/RW-39/PM/w39a/verify-layout.mjs
ok: layout.json — 65 блоков с source, 22 ссылок на токены, экранов 4; rail 64, tree 240/311, header 49, composer 56, members 240/256, rows 26/48 + member 42; колонки 2047.8 ≈ 2048
```

Файлы итерации 2 (md5 на момент отчёта):

| Файл | md5 |
|---|---|
| `waves/RW-39/DESIGN/layout.json` | `e5b2dcdfa5b7b60da6d7d446c9785994` |
| `waves/RW-39/DESIGN/README.md` | `8fb0ffff076f0a845fb714bc13606e6b` (до правки чисел 205 → см. md5sum) |
| `waves/RW-39/DESIGN/REFERENCES.md` | см. md5sum (обновлён §3) |
| `waves/RW-39/DESIGN/tokens.json` | производный (владелец регенерации W39b2; W39a не правил) |

Остаточные риски:

1. `tokens.json` — производный файл: любая правка `tokens.css` требует `gen-tokens.mjs`;
   RE-замеры палитры/типографики в него не добавляются (будут перезаписаны) — они в
   `REFERENCES.md` §3 и `layout.json`.
2. Расхождения `discrepancies[]` (эмодзи-cell 48 vs 44, switch 48 vs 40, settings-nav 264 vs 232,
   notice 34 vs 24, members-тулбар, settings max-w 740 vs 720) остаются за W39b/PM: W39a их
   только фиксирует с точкой замера.
3. Документы волны менялись параллельно (SPEC-P3-02, RE-EXTRA, FUNCTION-MATRIX); DESIGN-файлы
   W39a синхронизированы только по своим четырём артефактам.
