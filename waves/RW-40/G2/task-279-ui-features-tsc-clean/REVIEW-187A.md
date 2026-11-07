# REVIEW — TR-187A / PR-158 / task-279 (`FE-FIX-03`: `ui/features` tsc clean)

- **Ревьюер:** Технический ревьюер (DeepSeek, TR-187A)
- **Ветка:** `bugfix/task-279-ui-features-tsc-clean` (HEAD `355be21`)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED**
- **Дефекты:** `P0=0`, `P1=0`, `P2=2`, `P3=3`
- **Мутации:** **5/5 убиты** профильными unit-тестами, исходники восстановлены, MD5 совпали с эталоном ТЗ

## 1. Проверяемые файлы

| Файл | MD5 (эталон ТЗ) | MD5 (факт, до и после мутаций) | Статус |
|---|---|---|---|
| `rust/gui-spike/ui/features/attachment-validator.js` | `dec89713e4893572f6dba02f8dd2a4c9` | `dec89713e4893572f6dba02f8dd2a4c9` | совпал |
| `rust/gui-spike/ui/features/inbox.js` | `e1d84a89972594d8ed7c1b7b383ac567` | `e1d84a89972594d8ed7c1b7b383ac567` | совпал |
| `rust/gui-spike/ui/features/invite-parser.js` | `1835d4d322d9be4772998494390a7e7b` | `1835d4d322d9be4772998494390a7e7b` | совпал |
| `rust/gui-spike/ui/features/svg-icons.js` | `4790e8f94cda67280aa4b1befc095d24` | `4790e8f94cda67280aa4b1befc095d24` | совпал |
| `rust/gui-spike/ui/features/token-map.js` | `1781779a33804706b07d550199549fc8` | `1781779a33804706b07d550199549fc8` | совпал |

Диф — 16 добавленных и 9 удалённых строк на пять файлов, тесты не правились (`git status` не показывает изменений в `tests/`). Это важно: оракулом остались те же 83 теста из `task-187`, подгонки тестов под код нет.

## 2. Scope и `git status`

```
 D node_modules
 M package-lock.json
 M rust/gui-spike/ui/features/attachment-validator.js
 M rust/gui-spike/ui/features/inbox.js
 M rust/gui-spike/ui/features/invite-parser.js
 M rust/gui-spike/ui/features/svg-icons.js
 M rust/gui-spike/ui/features/token-map.js
```

Правки кода ровно в пяти файлах ТЗ. Две записи — состояние окружения, не работа воркера (то же самое уже зафиксировано в `REVIEW-183A.md` §2):

- `D node_modules`: в индексе симлинк (`120000`), в рабочем дереве — реальный каталог, `git` видит typechange как удаление.
- `M package-lock.json`: `+6` строк — `bin.discord-lab` и `engines.node >=24`, которые уже есть в `package.json`; локфайл догоняет манифест после `npm install`.

Ни то, ни другое в коммит PR-158 не включать.

## 3. Гейты

| Проверка | Команда | Результат |
|---|---|---|
| Typecheck по 5 файлам | `npx tsc -p tsconfig.json --noEmit 2>&1 \| grep -E "attachment-validator\.js\|inbox\.js\|invite-parser\.js\|svg-icons\.js\|token-map\.js"` | `0` строк, `grep` exit `1` |
| Unit-тесты пяти модулей | `node --test tests/unit/{attachment-validator,inbox,invite-parser,svg-icons,token-map}.test.mjs` | `tests 83, suites 11, pass 83, fail 0`, exit `0` |

Оба прогона повторены после мутационного раунда — те же числа. Полный `tsc` по репозиторию даёт **139** ошибок: все, кроме одной (см. P2-1), в `tests/unit/*`, которые в эту карточку не входят.

Нюанс, который стоит держать в голове: `tsconfig.json` (`include`: `scripts/`, `src/`, `bin/`, `tests/`) не перечисляет `rust/gui-spike/ui/**`. Эти пять модулей проверяются транзитивно — через импорт из unit-тестов (`allowJs`/`checkJs`). То есть из тестов нельзя удалить импорт модуля, не потеряв и его typecheck.

## 4. Что именно исправлено: 10 ошибок и доказательство необходимости каждой правки

Проверял не «на слово»: по очереди возвращал в рабочее дерево HEAD-версию одного файла (`git show HEAD:<path>`) и смотрел, что скажет `tsc`. Каждый файл вернул ровно те ошибки, которые PR закрывает:

| Файл | Ошибки на HEAD-версии | Код |
|---|---|---|
| `attachment-validator.js` | `47,10` | TS2532 `split(';')[0]` |
| `inbox.js` | `83,26` | TS7053 индексация литерального объекта строкой |
| `invite-parser.js` | `33,25`, `34,18` | TS2345 ×2 `match[1]` |
| `svg-icons.js` | `74,26`, `76,3`, `91,19`, `91,55`, `104,52` | TS2345, TS2739, TS2339 ×3 |
| `token-map.js` | `75,3` | TS2322 `string \| undefined` |

Итого 10 ошибок — совпадает с заявленным в `REPORT.md` red-прогоном. `@ts-ignore`, `@ts-expect-error`, `@ts-nocheck` в файлах нет (`grep` пуст), единственный каст — `unknown → SVGElement` в `svg-icons.js:66`, с комментарием и обоснованием.

## 5. Проверка эквивалентности поведения (old vs new)

Главный риск PR такого типа — «починили типы, попутно поменяли рантайм». Прогнал HEAD-версии и правленые бок о бок на одних и тех же входах:

| Модуль | Кейсов | Расхождений | Что сравнивалось |
|---|---|---|---|
| `attachment-validator.js` | 125 | 0 | `classifyAttachmentKind` на 11 `contentType` × 9 имён: `undefined`, `null`, `42`, `''`, `'IMAGE/PNG'`, `' image/png '`, `'image/png; charset=utf-8'`, `';x'`, `'  ;  '` |
| `invite-parser.js` | (в тех же 125) | 0 | `extractInviteCodes`: дубликаты, 4 ссылки при лимите 3, битые и нестроковые входы |
| `token-map.js` | (в тех же 125) | 0 | `resolveClientToken` (`'toString'`, `'__proto__'`, `'--BG-0'`, `7`), `auditStylesheetTokens` (пустой CSS, коллизии, `null`) |
| `svg-icons.js` | 204 | 0 | `createIconSvg`: все 20 имён + `'nope'`, `'Home'`, `'toString'`, `'__proto__'` × 8 наборов опций; `upgradeGlyphIcons` на `Element` и на `Document`, включая повторный вызов (идемпотентность) |

Сравнивалась сериализованная DOM-модель и возвращаемые значения. Пусто в графе расхождений — поведение не сдвинулось.

`inbox.js` проверял отдельно: правка — только JSDoc плюс фолбэк `?? tab`. Ключи подписей и значения `INBOX_TABS` совпадают один в один (`tabsWithoutLabel=0`), то есть фолбэк в рантайме недостижим; `tsc` без него тоже чист (см. T2 в §6).

## 6. Мутации

### 6.1. Основной раунд: по одной мутации в каждом файле

Каждая мутация — точечная замена одного выражения, прогон профильного теста, восстановление из бэкапа и сверка MD5. Логи — `mutations/M*.log`.

| ID | Файл | Мутация | MD5 мутанта | Тест | Итог | Убивший тест |
|---|---|---|---|---|---|---|
| M1 | `attachment-validator.js` | `classifyAttachmentKind`: ветка `image/*` возвращает `'file'` | `863ff7cc59b28c7fe392c16f5664e45e` | `tests/unit/attachment-validator.test.mjs` | **убита** (`fail 1`, pass 20/21) | «classifyAttachmentKind detects image by extension and content type» — `actual: 'file'` |
| M2 | `inbox.js` | `INBOX_TABS` урезан до `["for_you","unreads"]` | `b8863b1094a647c90820f904a5753dac` | `tests/unit/inbox.test.mjs` | **убита** (`fail 2`, pass 17/19) | «INBOX_TABS» и «createInboxController: DOM-каркас» |
| M3 | `invite-parser.js` | убрана дедупликация `!codes.includes(code)` | `55dc4413299b810ca9b99c48fe71ad1e` | `tests/unit/invite-parser.test.mjs` | **убита** (`fail 1`, pass 13/14) | «дедупликация с сохранением порядка первого вхождения» |
| M4 | `svg-icons.js` | `viewBox` собран из `size` вместо `ICON_SIZE` | `89291b35249203ddfcf16787a1eb83e5` | `tests/unit/svg-icons.test.mjs` | **убита** (`fail 1`, pass 13/14) | «принимает размер и className из options» — ждали `0 0 20 20`, получили `0 0 32 32` |
| M5 | `token-map.js` | при неизвестном токене возвращается сам токен вместо `null` | `e54aceacb6b2e1eb3b270e4c4047aa53` | `tests/unit/token-map.test.mjs` | **убита** (`fail 3`, pass 12/15) | «возвращает null для неизвестного токена» и два теста `auditStylesheetTokens` |

Все пять убийств — `AssertionError` из нужного файла, а не падение импорта. MD5 после каждого восстановления и после всего раунда совпал с эталоном ТЗ.

### 6.2. Дополнительный раунд: мутанты ровно на строках дифа

Этот PR — про типы, поэтому отдельно проверил, чем именно защищена каждая правленая строка (`mutations/T*.tsc.log`, `T*.test.log`):

| ID | Правленая строка | `tsc` по файлу | Unit-тест | Вывод |
|---|---|---|---|---|
| T1 | `normalizeContentType`: убрано `.toLowerCase()` | 0 ошибок | `21/21` pass | **выжил** — нормализация MIME (регистр) тестами не покрыта → P3-2 |
| T2 | `inbox.js`: убран фолбэк `?? tab` | 0 ошибок | `19/19` pass | **выжил** — фолбэк лишний и недостижимый → P3-1 |
| T3 | `invite-parser.js`: убран `typeof code === 'string'` | 2 ошибки (TS2345 ×2) | `14/14` pass | убит `tsc` |
| T4 | `svg-icons.js`: убран каст к `SVGElement` | 1 ошибка (TS2739) | `14/14` pass | убит `tsc` |
| T5 | `token-map.js`: убран `?? null` | 1 ошибка (TS2322) | `15/15` pass | убит `tsc` |

Вывод по методу: три правки из пяти — чисто типовые (их стережёт `tsc`, unit-тесты к ним равнодушны), две — рантайм-фолбэки, которые ничем не покрыты. Для типовой задачи это нормально, но и выжившие мутанты, и оставшаяся ошибка в `thread-badge.js` показывают, что «ui/features tsc clean» ещё не закрыт.

## 7. Замечания

### P2-1. В `thread-badge.js` осталась ошибка того же класса — цель «`ui/features` tsc clean» не достигнута

`rust/gui-spike/ui/features/thread-badge.js(75,5): error TS2322: Type 'string \| undefined' is not assignable to type 'string'` — это `THREAD_ARCHIVE_DURATIONS_RU[normalizeArchiveDurationMinutes(...)]`, ровно тот же паттерн «индексная выдача `string | undefined`», который PR-158 чинил в пяти файлах. Файл в заявленный scope не входит (ТЗ перечисляет пять файлов, и `thread-badge.test.mjs` уже существует), поэтому merge не блокирую. Нужна отдельная карточка: после неё `tsc` по `ui/features` станет действительно нулевым. Сейчас это единственная ошибка вне `tests/`.

### P2-2. Шум рабочего дерева (`node_modules`, `package-lock.json`)

Симлинк `node_modules` в рабочем дереве заменён реальным каталогом, `package-lock.json` рассинхронизирован с `package.json`. Воспроизводится в несвязанных деревьях (см. §2 и `REVIEW-183A.md` §2), к PR-158 отношения не имеет, но при merge в `unstable` включать в коммит только пять файлов задачи.

### P3-1. `inbox.js`: фолбэк `?? tab` не нужен ни типам, ни рантайму

Убрал его (мутант T2) — `tsc` чист, тесты зелёные; в рантайме ветка недостижима, потому что ключи `INBOX_TAB_LABELS` и значения `INBOX_TABS` совпадают. Либо убрать строку, либо оставить как страховку от дрейфа словаря, но тогда закрыть тестом: подпись вкладки должна быть русской, а не техническим `for_you`.

### P3-2. Тест не проверяет нормализацию MIME

`normalizeContentType` приводит `contentType` к нижнему регистру и срезает параметры, но все тесты передают уже нормализованные строки (`'image/png'`, `'image/webp'`). Мутант без `.toLowerCase()` выживает в `21/21`. Достаточно двух кейсов в `attachment-validator.test.mjs`: `classifyAttachmentKind('blob', 'IMAGE/PNG') === 'image'` и `classifyAttachmentKind('blob', ' image/png ; charset=utf-8') === 'image'`.

### P3-3. `svg-icons.js`: двойная и неисполняемая защита, двойной каст

- `const pathData = SVG_ICONS[name]; if (pathData === undefined) return null;` — после `hasOwnProperty` ветка недостижима, индекс уже сужен. Вреда нет, но тестом не покрыта и не покроется.
- `/** @type {SVGElement} */ (/** @type {unknown} */ (doc.createElement("svg")))` маскирует реальный тип. Рантайм подтверждён (204 кейса диф-теста, `tagName === 'SVG'`), но честнее `doc.createElementNS("http://www.w3.org/2000/svg", "svg")`; отложено сознательно — это меняет требования к DOM-шиму в `svg-icons.test.mjs` и в `ui/features/attachments*.js`-подобных местах.

## 8. Что осталось за пределами ревью

- Полный `npm run gate:worker` не запускался: ТЗ TR-187A ограничивает проверки targeted-прогоном пяти тестов и `tsc`. Полный гейт делает integration-лейн на batch.
- `git commit`/`push` не выполнялись (запрещено ТЗ).
- Оставшиеся 138 ошибок `tsc` — все в `tests/unit/*`; это карточки `task-280…282`.

## 9. Итог

PR-158 закрывает ровно те 10 ошибок `tsc`, которые заявлены в red-прогоне, не меняя рантайм-поведение: диф-тест old vs new на 329 кейсах дал 0 расхождений, `@ts-ignore`-заглушек нет, тесты не правились. Пять точечных мутаций убиты профильными тестами, файлы восстановлены побайтно. Блокирующих дефектов нет.

**Вердикт: APPROVED** (`P0=0`, `P1=0`, мутации `5/5`). P2-1 и P2-2 не блокируют merge в `unstable`, но P2-1 стоит завести карточкой, иначе следующая волна снова увидит одну и ту же ошибку в `ui/features`.
