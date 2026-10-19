# Discord 1.0.162 — i18n: рантайм, плурализация, rich-формат

Закрытие пробела «плурализация/rich-формат» по локальным артефактам Discord Desktop 1.0.162 (Linux, flatpak).
Все факты — из распакованных бандлов и каталогов; каждая цитата проверена скриптом `i18n-deep-build.mjs`
(падает, если фрагмент не найден по смещению). Машиночитаемая версия — `i18n-deep.json`.

## 0. Источники и что нового относительно `docs/`

| Что | Значение |
|---|---|
| Бандлы | `/home/sanya/re-deep/out/bundles` — 3 628 файлов, 61 МБ |
| i18n-рантайм + simple-markdown | `discord.com_assets_501428.98d57a9bd8cbb604.js` (12,7 МБ; модули 237341/624067/668317/446274/618830/239127/835751) |
| Правила сообщений Discord | `discord.com_assets_web.4d4b2f75fdea5470.js` (12,4 МБ; модули 652215/999915/247186/379418/113051/7584/873879/223637/683412/704726) |
| Каталоги | `ru-catalog.json` — 23 836 ключей / 2 443 841 Б; `en-catalog.json` — 23 903 / 1 427 037 Б |

`docs/discord-qt-analysis.md` и `docs/vesktop-analysis.md` фиксируют только сам факт локализации
(«RU-каталог 26 754 сообщения») и прямо помечают: «механизм плюрализации не разобран».
В `docs/` нет `i18n.md`. Новое здесь: рантайм и его модули, плурализация с примерами ru/en,
синтаксис разметки сообщений с регулярками, форматы аргументов, список локалей и fallback,
единицы/даты, ограничения. Цифра 26 754 на 1.0.162 не воспроизводится: в кэше 23 836 ключей
(32 495 строк) — расхождение с более старой выгрузкой, отдельно не разбиралось.

## 1. Рантайм: свой AST, не IntlMessageFormat

Реализация — **собственная** (`FormatBuilder` + `bindFormatValues`), поверх `Intl.*` (не formatjs/ICU-парсер).
Ключевые цитаты (файл:смещение — здесь и далее смещение в байтах от начала бандла):

- `239127(e,t,r){"use strict";var n=this&&this.__awaiter` — модуль `MessageLoader`
  (`501428.98d57a9bd8cbb604.js:17458`), `_loadLocale(e){...}` (:19797), `localeImportMap[e]()` (:20259).
- `668317(e,t,r){...t.FormatBuilder=void 0` (:5284) — `bindFormatValues`/`FormatBuilder`.
- `835751(...)t.IntlManager=t.DEFAULT_LOCALE=void 0` (:15943) и `t.DEFAULT_LOCALE="en-US"`.
- `dataFormatterCache=new class{...this.dateTime=new Map` (:1995, модуль 237341) —
  кэш `Intl.DateTimeFormat / DurationFormat / ListFormat / NumberFormat / PluralRules / RelativeTimeFormat`.
- `formatNumber(r,a){let s=...resolveFormatConfigOptions)(t.number,a);...getNumberFormatter` (:4369, модуль 624067).

Схема: `createLoader(localeImportMap, defaultLocale)` → `supportedLocales = Object.keys(map)` (:18297) →
при обращении к локали `_loadLocale` делает динамический `import()` соответствующего чанка
(`localeImportMap[e]()`, :20259) → `this.messages[e] = (yield o).default` → значение
оборачивается в `InternalIntlMessage(ast, locale)`, парсится один раз и кэшируется по локали.

**Ключи сообщений — хешированные, 6 символов** (`446274`, :12888):
`runtimeHashMessageKey(s)` = `base64(xxh64(s, seed=0) в LE-порядке)[0..6]`. Проверено 14/14
против клиентских модулей (`ru → +irF5O`, `en-US → +dXJII`, `AKA → l1QVfj`, `OK → TyCVIq`, …);
сам xxh64 (модуль `618830`) сверен с `xxhsum -H1` на 81 входе — 0 расхождений.
Важно: ключ считается от **внутреннего id**, а не от английского текста — из 22 803 en-строк
hash(текст) совпал с ключом только 5 раз (AKA/GIF/OK/PS4/PS5); SCREAMING_SNAKE-иды — 1 246 (5.46%).
Поэтому «восстановить ключ по тексту» нельзя, а искать переезд строк нужно по значению AST.

Версия AST — **keyless JSON**, типы узлов (`524897`): `0 Literal, 1 Argument, 2 Number, 3 Date,
4 Time, 5 Select, 6 Plural, 7 Pound (#), 8 Tag`. Пример: `[[2,"size"]," МБ"]`, `[3,"timestamp","medium"]`.
Отдельно есть `FORMAT_JS_POUND=Object.freeze({type:7})`.

Форматтеры чисел/дат/времени — из `DEFAULT_FORMAT_CONFIG` (модуль `896745`):
`number:{integer:{maximumFractionDigits:0},currency:{style:"currency"},percent:{style:"percent"}}`,
`date.short/medium/long/full`, `time.short/medium/long/full`.

## 2. Плурализация

Механизм (модуль `668317`): узел `[6,"count",{ветки},offset,"cardinal"|"ordinal"]`;
сначала точный ключ ``=`${value}` ``, иначе `Intl.PluralRules(type).select(value - offset)`,
затем `` .other ``; если ветки нет — исключение
`"${b} is not a known option for plural value ... Valid options are ..."` (:8001).
`#` (Pound, `[7]`) внутри ветви подставляется через `Intl.NumberFormat` (`formatNumber(h)`).

Категории ru (`Intl.PluralRules("ru")`, CLDR): **one** — 1, 21, 31…; **few** — 2–4, 22–24…;
**many** — 0, 5–20, 25–30…; **other** — дроби; плюс точные селекторы `=N`.
В каталоге ru встречаются ветви: `other` 965, `one` 878, `few` 689, `=0` 107, `=1` 38, `many` 28,
`=2` 18, `=-1` 7, `=3` 1, `=6` 1. В en ветвей `few/many` нет.

Плуральных ключей: ru 911 (3.82%), en 932. Плуральных узлов в ru: 965.

Живые примеры (пересчитаны `Intl.PluralRules` и сверены с `plural-demo.txt`):

| Ключ | ru-шаблон (сокращённо) | 1 | 2 | 5 | 11 | 21 | 22 |
|---|---|---|---|---|---|---|---|
| `+yF0ds` | `{count, plural, =0{…} one{Выбран # получатель} few{Выбрано # получателя} many{Выбрано # получателей} other{…}}` | Выбран 1 получатель (one) | Выбрано 2 получателя (few) | Выбрано 5 получателей (many) | — | Выбран 21 получатель (one) | Выбрано 22 получателя (few) |
| `r/EUap` | `[6,"count",{one:«# игра»,few:«# игры»,many:«# игр»,other:«# игр»}]` | 1 игра | 2 игры | 5 игр | 11 игр (many) | 21 игра | 22 игры |
| `+sThav` | `[6,"hours",{one:«# час»,few:«# часа»,many:«# часов»,other:«# часа»}]` | 1 час | 2 часа | 5 часов | — | 21 час | — |
| `pAQJsJ` | `[6,"count",{one:«# участник»,few:«# участника»,other:«# участников»}]` | 1 участник | 2 участника | 5 участников | 11 участников | 21 участник | 22 участника |
| `C4WXvc` | `[6,"memberCount",{one:«# человек »,other:«# чел.»}]` | 1 человек | 2 чел. | 5 чел. | — | — | — |

en-контраст: `+yF0ds` = `{count, plural, =0{No recipients selected} one{# recipient selected} other{# recipients selected}}`,
`pAQJsJ` = `one{# Member} other{# Members}`, `C4WXvc` = `one{# Person} other{# People}` — то есть
перенос «one/other» на русский без ветвей few/many даёт неверные формы («2 участник»).

Ловушки (проверено на каталоге):
- `other` в ru обязателен и часто равен many-форме (`r/EUap`: 5 → «игр», other = «игр»);
- ветви `many` в ru может просто не быть (`pAQJsJ`, `C4WXvc`) — тогда 5…20 идут в `other`;
- `few` покрывает 2–4 **и** 22–24 (не только «2–4»);
- точные селекторы бывают не только `=0/=1`: в `d8pkUP` есть `=2` и `=3`;
- внутри вложенных плуралов `#` берётся из своей ветви (`currentPluralValue`);
- отсутствие нужной ветви — не «пусто», а throw (см. §5).

## 3. Rich-формат сообщений (markdown)

Парсер — **simple-markdown** (движок в `501428`, правила Discord — в web-бандле), выход — React-узлы,
не HTML-строка. Полный список правил с регулярками (файл:смещение):

| Синтаксис | Смысл | Регулярка/правило |
|---|---|---|
| `**текст**` | жирный | `/^\*\*((?:\\[\s\S]|[^\\])+?)\*\*(?!\*)/` · `501428:824318` |
| `__текст__` | подчёркнутый | `/^__((?:\\[\s\S]|[^\\])+?)__(?!_)/` · `501428:824595` |
| `~~текст~~` | зачёркнутый | Discord-переопределение: `/^~~([\s\S]+?)~~(?!_)/` → `<s>` · `web:5891002` |
| `*текст*`, `_текст_` | курсив | `em` c `quality = length+0.2` · `501428:823844` |
| `` `код` `` | инлайн-код | `/^(`+)([\s\S]*?[^`])\1(?!`)/` · `501428:825091` |
| ` ```lang … ``` ` | блок кода | `/^```(?:([a-z0-9_+\-.#]+?)\n)?\n*([^\n][^]*?)\n*```/i` (+ hljs, `dangerouslySetInnerHTML`) · `web:5887232` |
| `\|\|текст\|\|` | спойлер | `ie=/^\|\|([\s\S]+?)\|\|/` (require first char `\|`) · `web:5891144`, `web:314350` |
| `> текст` | цитата | `/^( *>[^\n]+(\n[^\n]+)*\n*)+\n{2,}/`, снятие `^ *> ?` · `501428:819067` |
| `#`/`##`/`###` | заголовок 1–3 | `/^ *(#{1,3})(?:\s+)((?!\s*#{1,3}\s)[^\n]+?)#*\s*(?:\n|$)/` (нужен `allowHeading`) · `web:5865874` |
| `-# текст` | малый текст | `/^ *-# +((?!-#)[^\n]+)(?:\n|$)/`, флаг `allowSubtext` · `web:5894028` |
| `-`, `*`, `1.` | список (глубина ≤ 11) | `/ *-0123456789/`, `t._listLevel>=11 → null` · `web:5870466` |
| `@silent` | без уведомления | `/^(@silent(?![^\s]))/` · `web:5889046` |
| `<@123>`, `<@!123>`, `@everyone`, `@here` | упоминание | `/^<@!?(\d+)>\|^(@(?:everyone\|here))/` · `web:5888290` |
| `<@&123>` | роль | `/^<@&(\d+)>/` · `web:5887472` |
| `<#123>` | канал | `/^<#(\d+)>/` · `web:5882377` |
| `</команда опц:123>` | слэш-команда | `/^<\/((?:[-_\p{Letter}\p{Number}\p{sc=Deva}\p{sc=Thai}]{1,32})…):(\d+)>/u` · `web:5889412` |
| `<a:name:123>`, `<:name:123>` | кастомный эмодзи | `/^<(a)?:(\w+):(\d+)>/` (валидный id → `\d{17,19}`) · `web:5883764`, `web:314350` |
| `:name:` `:name::skin-tone-3:` | эмодзи по имени | `EMOJI_NAME_RE=/^:([^\s:]+?(?:::skin-tone-\d)?):/` · `web:4453876` |
| `<t:1712345678:R>` | таймстамп | ``^<t:(-?\d{1,17})(?::(t\|T\|d\|D\|f\|F\|s\|S\|R))?>`` · `web:5903524` |
| `<sound:guildId:soundId>` | саундборд | `/^<sound:(\d+):(\d+)>/` · `web:6975997` |
| `<id:home>`, `<id:browse:123>` | статический маршрут | `/^<id:(home\|browse\|customize\|guide\|linked-roles)(?::(\d+))?>/` · `web:314350` |
| `https://…`, `<https://…>`, `[t](u)` | ссылки | `/^(https?:\/\/[^\s<]+[^<.,:;"')\]\s])/`, `<([^: >]+:\/[^ >]+)>` · `501428:822642` |
| `\* \_ \~ \| \\` | escape | `/^\\([^0-9A-Za-z\s])/` → литерал · `501428:821885` |

Порядок применения (`order` в правилах; движок сортирует по `order`, разрешает ничьи по `quality`,
`501428` — сортировка правил): `heading 0 → nptable → lheading → hr → codeBlock(отступ) → fence →
blockQuote 6 → list 7 → def → table → newline 10 → paragraph 11 → escape 12 → tableSeparator →
autolink 14 → mailto → url 16 → link 17 → image…refimage → em 21 → strong 22 → u 23 → del 24 →
inlineCode 25 → br 26 → text 27`. Discord-правила (mention/emoji/spoiler/timestamp/…) получают
`order` текстового правила (последний) и включаются по `requiredFirstCharacters` (например, `["<"]`),
то есть перекрывают `text` не «раньше», а точнее.

**Экранирование — порядок важен:**
1. HTML-экранирование НЕ делается до разметки: парсер работает по сырому тексту, а рендер — React-узлы.
   `sanitizeText` (`E={"<":"&lt;",">":"&gt;","&":"&amp;"…}`, `501428:815303`) применяется только в
   HTML-выходе движка (`defaultHtmlOutput`), который Discord для сообщений не использует.
2. Markdown-escape `\X` — самостоятельное правило (убирает спецсимвол, порядок 12, до em/strong/u/del/inlineCode).
3. URL-санитайзер: `sanitizeUrl` запрещает `javascript:`, `vbscript:`, `data:` (`501428`).
4. Блоки кода уходят в hljs и вставляются через `dangerouslySetInnerHTML` — экранирование там на стороне hljs.

Редактор (Slate, `web`): для подсветки синтаксиса известен полный список типов правил и
«синтаксических» символов — `A=new Set(["*","_","~","|","\\"])`, пары для инлайн-стилей:
`strong **`, `u __`, `s ~~`, `spoiler ||`, `em *`, `inlineCode` = одиночный бэктик, `subtext "-# "`,
`autolink/mailto/tel "<…>"`, `soundboard "<sound:…>"`, `staticRouteLink "<id:…>"`,
`silentPrefix "@silent "`, `escape "\"` (`web:3460477`).
Там же маппинг «verbatim/inlineObject/inlineStyle/skip» для каждого правила — удобная таблица
для своего парсера-редактора.

## 4. Rich-теги в i18n (не markdown сообщений)

В значениях каталога теги — это узел `[8,"$b",children,control]`; набор из шести тегов
(`DEFAULT_REACT_RICH_TEXT_ELEMENTS`, `501428`):

| Тег | React | markdown-вывод | AST |
|---|---|---|---|
| `$b` | `strong` | `**…**` | `strong` |
| `$i` | `em` | `*…*` | `em` |
| `$del` | `del` | `~~…~~` | `s` |
| `$code` | `code` | `` `…` `` | `inlineCode` |
| `$link` | `a[href]` | `[текст](url)` | `link{target}` |
| `$p` | `p` | `текст\n\n` | `paragraph` |

Неизвестный тег — throw `` `${e} is not a known rich text formatting tag` ``.
Тег без `$` — **функция-хук**: получает `(nodes, keyPrefix)` и возвращает узлы
(примеры: `F6Hook`, `tabHook`, `usernameHook`); у `$link` третий элемент — control-узлы
с колбэками (`[[1,"onSettingsClick"]]`, `[[1,"helpCenterLink"]]`). В ru 2 515 ключей с тегами,
3 736 Tag-узлов.

## 5. Аргументы и форматы данных

- Подстановка: `{name}` — узел `[1,"name"]`. Отсутствующий аргумент (и не тег `$…`) — **throw**
  `No value for variable '<v>' was provided for the localized message '<m>'` (класс с
  `variableName/originalMessage/nodeType`); тихого фолбэка нет.
- `select`: `[5,"var",{...}]` — прямое сравнение; нет ветки и нет `other` → throw
  `"… is not a known option for select value …"`.
- `#` (Pound): `[7]` → `Intl.NumberFormat` от текущего плурального значения.
- Числа: `[2,"name"]` → `Intl.NumberFormat` с конфигом; **скелеты чисел в каталоге не используются**
  (все Number-узлы 2-элементные: например `[2,"size"]`, `[2,"percent"]`, `[2,"minutes"]`),
  поэтому «%» и единицы — обычные литералы в строке.
- Даты/время: `[3,"name","medium"]` → `Intl.DateTimeFormat`; скелеты в ru: `short`, `medium`, `long`,
  а также произвольные `M/d` и `::MMMMd`; времени — `short`. В сообщениях относительное время
  представлено не `RelativeTimeFormat`, а `<t:…:R>` (dayjs `fromNow()` с настраиваемыми порогами
  и `setInterval`-обновлением).
- Списки — `Intl.ListFormat` (в т.ч. `formatListToParts` с placeholder-подстановкой).
- Длительности — `Intl.DurationFormat`, но забавный факт: `formatDuration` берёт конфиг
  `resolveFormatConfigOptions(t.time, a)` (раздел `time`, не `duration`).

## 6. Числа, единицы, «N участников»

- Размеры файлов — число + переведённый суффикс: en `bTzRR6 [[2,"size"]," KB"]`, `pIn7Af " MB"`,
  `yhEXX7 " GB"`, ru — `[[2,"size"]," МБ"]`, `" ГБ"`, `" КБ"`; скорости: `"кбит/сек"`, `"мбит/сек"`.
  То есть **конвертацию байт→единицы делает вызывающий код**, i18n только форматирует число
  (никакого SI/IEC-выбора в строках нет).
- Лимиты: `DYFPg2 ["… Максимальный общий размер — ",[1,"maxSizeMb"]," МБ."]` — аргумент уже в МБ;
  `kIO9jy ["… Размер не должен превышать ",[1,"maxSize"]," Кб."]` — аргумент (не Number-узел).
- Проценты: `[2,"percent"]` + литерал `"%"` (`4FUWIL`: «Получите скидку {percent}% на Nitro…»).
- Разделитель разрядов ru — `U+00A0`: `Intl.NumberFormat("ru").format(1234567) = "1 234 567"`.
- «N участников»: `pAQJsJ` (one/few/other), `C4WXvc` (one/other, «чел.»), `u2qHzE`/`+yF0ds`
  («Выбран N получатель»).

## 7. Локали, выбор языка, fallback

- Список в ассете — 36 кодов (`lang-list.json`): `en-US en-GB zh-CN zh-TW cs da nl fr de el hu it ja
  ko pl pt-PT pt-BR ru sk es-419 es-ES sv-SE tr bg uk fi no hr ro lt th vi hi he ar id`.
- Включённых — **31**: код локали хешируется тем же `runtimeHashMessageKey` и находится в ru-каталоге.
  Отключены: `pt-PT, sk, he, ar, id`.
- Fallback (модуль `239127`): запрошенная локаль → `DEFAULT_LOCALE` (`en-US`) → цепочка
  `fallbackWith(...)` (с защитой от цикла) → предупреждение
  `Requested message <debugKey> (<key>) does not have a value in the requested locale <ru> nor
  the default locale <en-US>` и **пустое сообщение** (`fallbackMessage`).
- `IntlManager` создаёт форматтеры сразу на пару `[currentLocale, defaultLocale]`
  (при `forceLookupMatcher` добавляется `localeMatcher:"lookup"`).
- Ограничение: полного каталога `uk` в локальных кэшах нет (каталог только ru/en-US);
  `uk` представлен legacy named-key модулем — 58 ключей.

## 8. Шум и что вырезано

- Каталог ru целиком в отчёт не выносился: 23 836 ключей, 2 443 841 Б JSON, 32 495 строк.
- Из 3 628 бандлов разобраны 2 (i18n-рантайм и правила сообщений); остальные 61 МБ не описывались.
- В ru-каталоге 259 значений (0.9%) побайтово совпадают с en (бренды/числа/непереведённое).
- Тексты других локалей в кэше отсутствуют — сверка только ru/en.

## 9. Unknowns / ограничения

1. Ключи-хеши непрозрачны (id ≠ текст): 5 совпадений из 22 803, переезд строк отслеживается по AST.
2. Полного uk-каталога нет; проверялся только локальный кэш (3 796 файлов code cache + 3 785 записей).
3. Правило `highlight` в рендере сообщений отключено (`match:()=>null`), в редакторе — есть.
4. Не разбиралось, кто и когда вызывает `loadAllMessagesInLocale` до первого рендера.
5. Серверные/embed-рендеры и поиск не проверялись (только локальный web-бандл 1.0.162).

## 10. Выводы для своего клиента

1. **Формат:** keyless JSON-AST с 9 типами узлов + 6 rich-тегов. Ключи делать прозрачными
   (например `channel.members.count`), но предусмотреть хеш-слой, если понадобится совместимость
   с ключами Discord (base64(xxh64(id)) — 6 символов).
2. **Плурализация ru:** ветви `one/few/many/other` + точные `=N`; `other` обязателен и должен быть
   корректной формой (в Discord он часто = many), `#` форматировать `Intl.NumberFormat(locale)`.
   Перенос en-шаблонов `one/other` на русский даёт ошибки вида «2 участник».
3. **Разметка:** брать порядок правил simple-markdown (escape 12 → inline-стили 21–25 → text 27),
   учитывать `requiredFirstCharacters`, ограничения Discord (`#` только 1–3, списки ≤ 11 уровней,
   `-#` для малого текста, `@silent`), и не путать два rich-слоя: markdown пользователя ≠ i18n-теги `$b/$link`.
4. **Экранирование:** HTML-escape не до разметки; в React-пути он вообще не нужен, в HTML — после парсинга
   (`sanitizeText`), ссылки фильтровать (`javascript:/vbscript:/data:` запрещены), код — только через
   подсветчик с собственным экранированием.
5. **Данные:** числа/даты/списки — `Intl.*` с кэшем по (locale, опции); единицы (КБ/МБ/ГБ, кбит/сек)
   держать литералами в переводах, конвертацию — в коде; относительное время — `Intl.RelativeTimeFormat`
   или dayjs-совместимые пороги, с таймером обновления.
