# RW-39 / W39a — REFERENCES: референсы, что измерено, пробелы

Статус: Phase 1a (RE-артефакты). Дата: 2026-10-23. Автор: W39a (RE/REA). PM: DeepSeek.
Связанные файлы: `SPEC.md` (числа и правила), `tokens.css` (значения), `DOM-CONTRACT.md`
(разметка), `tokens.json` (машиночитаемые значения), `layout.json` (геометрия shell/экранов).

Референсы лежат в `/home/sanya/re-deep` и открывались **только на чтение**. В репозиторий
не скопировано ни одного PNG/JPG, шрифта, иконки или спрайта: переносятся только **числа**
(цвета, размеры, отступы). Бренд в UI — `DL`.

## 0. Коды статуса (совпадают с SPEC.md §1)

| Код | Значение | Чем подтверждается |
|---|---|---|
| `[M]` | измерено | пиксельный скан PNG/JPEG или `getBoundingClientRect` из DOM-выгрузки |
| `[T]` | токен | `/home/sanya/re-deep/ui/tokens.json` (4795 переменных, dark, 1.0.162) |
| `[L]` | RE-контракт | `waves/RW-37/RE/layout.json` (schema `rw37/re-layout-v1`) |
| `[A]` | assumption | выбрано нами, обоснование в SPEC.md; за измерение не выдаётся |
| `[F]` | frozen | заморожено тестами RW-38 (`tests/unit/gui-spike-dom.test.mjs`) |

Среда замеров: окно 2048×1106.4 CSS, `devicePixelRatio = 1.25` (`ui/log.json`), PNG 2560×1383.
Все числа ниже — **CSS px** (пиксели PNG / 1.25). Скрипты: `waves/RW-39/PM/w39a/measure.py`
(пиксели, `--json` → `measure.json`), `gen-tokens.mjs` + `verify-tokens.mjs` (токены),
`verify-layout.mjs` (геометрия).

## 1. Пиксельные референсы (PNG/JPEG)

| id | Путь | Что измерено | Метод | Статус |
|---|---|---|---|---|
| R-01 | `ui/00-app.png` | titlebar **32**; шапка чата **48 + 1px граница = 49**; граница дерево/чат x=**375.2** → дерево **311.2**; чат **1416.8**; composer **56** (y 1044…1100), ширина **1401.6** (x 382.4…1784); user bar **56**; строка канала **28**, шаг строк **30** | PIL: скан столбца x=1500 / строки y=900, поиск полос цвета `(34,35,39)`; медиана шага текстовых полос дерева (n=19) | `[M]` |
| R-02 | `ui/00-app.png` | цвета: рейл/дерево/titlebar `#121214`, чат и шапка `#1a1a1e`, user bar `#202024`, composer `#222327`, выбранный канал `#2c2c30`, акцентная плашка `#5865f2` | точечные пробы lossless PNG | `[M]` |
| R-03 | `ui/00-app.png` | статус-точки панели участников: dnd **`#da3e44`**, online **`#45a366`** (частотный скан насыщенных пикселей) | PIL: Counter по пикселям панели при `max−min>55` | `[M]` |
| R-04 | `ui/00-app.png` | полоса «20+ новых сообщений»: **32** CSS, `#5865f2`, начинается сразу под шапкой | скан столбца x=1500 | `[M]` |
| R-05 | `ui/05-dm-list.png` | размер кадра 2560×1383 (dpr 1.25), общий каркас shell; **список ЛС пуст** (`ui/05-dm-channels.json` = `[]`) | IHDR + `log.json` (`dm-list channels: 0`) | `[M]` для каркаса, содержимое — пробел (см. §7) |
| R-06 | `ui/06-profile-popout.png` | попаут профиля: bbox **268.8×408** при (22.4, 608.8); фон `#2c2d32` (= `--surface-4`), нижние панели `#242429` (= `--surface-2`) | PIL: bbox по цвету `(44,45,50)`, скан x<600 | `[M]` |
| R-07 | `ui/07-guild-menu.png` | захват **неудачный**: `log.json` `guild-menu items: 0`, `07-guild-menu.html` — 41 байт, картинка почти пустая | чтение `log.json` + размер html | пробел (см. §7) |
| R-08 | `ui8/22-members.jpg` | панель участников **256×1025** при (1792, 82); 35 строк, 6 групп; шаг строки участника **43.2 ≈ 42** (n=16, допуск ±1 из-за JPEG); аватар 32 | PIL: профиль «верхушки аватаров» по x 8…52, медиана шагов | `[M]` |
| R-09 | `ui8/20a-search-open.jpg`, `20a-search-typed.jpg` | поле поиска канала **244×28** (открытое состояние — 244×28, внутренний бар 242×26) | `20a-searchbar.json` (DOM rect) | `[M]` |
| R-10 | `ui8/20b-search-filters.jpg` | фильтры поиска: **7 секций** (От, В, Содержит, Упоминания, Дата, Тип автора, Закреплено), **5 input**, **4 кнопки**, **9 опций** «Содержит» | `20b-search-filters.json` (`labels/inputs/buttons`), `20b-search-filters-has.json` (`options`) | `[M]` |
| R-11 | `ui8/20c-search-sort.jpg` | меню сортировки: **3 пункта** (По новизне — `aria-checked=true`, Самое старое, Самое актуальное), `role="menu"` | `20c-search-sort.json` | `[M]` |
| R-12 | `ui8/21b-emoji-full.jpg` | пикер эмодзи: **42 ячейки** в сетке, **7** элементов nav, placeholder «Найдите идеальный эмодзи», 3 контент-вкладки (Гифки/Стикеры/Эмодзи) | `21b-emoji-full.json` (`gridCount/nav/searchPh/tabs`) | `[M]` |
| R-13 | `ui8/21d-emoji-categories.jpg` | **10 категорий** рейла (top guild, recent, guild, people, nature, food, activity, …) | `21d-emoji-categories.json` (`count=10`) | `[M]` |
| R-14 | `ui8/23-settings-voice.jpg` | раздел «Голос»: контент **1020×865** при (633, 121); **38** переключателей (`role="switch"`) | `23-settings-voice.json` (`rect`, `switchCount`) | `[M]` |
| R-15 | `ui8/23-settings-categories.jpg`, `23-settings-open*.jpg` | модалка настроек **1398×961** при (325, 73); **63** строки навигации (вложенные уровни) | `23-settings-categories.json` (`rect`, `count`) | `[M]` |
| R-16 | `ui/01-msg-menu.png` + `01-msg-menu-items.json` | контекстное меню сообщения: **15 пунктов** (4 реакции + 11 действий), `disabled` только у реакций | json-выгрузка | `[M]` для состава, геометрия — пробел |
| R-17 | `ui/03-quickswitcher.png` + `03-quickswitcher-results.json` | квик-свитчер: **8** результатов, ввод в contenteditable | json-выгрузка | `[M]` для состава, геометрия — пробел |
| R-18 | `ui/04b-search-results.png`, `04-search-open.png` | панель результатов поиска: «3 302 результата» (ProBot), «49 887» (Привет); `04b-search-results.json` пуст | `docs/re/frontend-d03.md` §1.1 + PNG | `[M]` для вида, данных нет (см. §7) |

## 2. DOM-выгрузки (`getBoundingClientRect`, CSS px)

| id | Путь | Что взято | Статус |
|---|---|---|---|
| D-01 | `ui/log.json` | окно `2048×1106`, `dpr 1.25`; `guild-rect w=64, h=40` (рейл-кнопка); `avatar-rect 32×32`; `home-rect 8×40`; `msg-rect h=66` (3×22); `tokens count=4795` | `[M]` |
| D-02 | `ui8/22-members.json` | панель `{x0:1792,y0:82,w:256,h:1025}`, `rowCount=35`, `groupCount=6` | `[M]` |
| D-03 | `ui8/23-settings-voice.json` | контент `{633,121,1020×865}`, `switchCount=38` (совпадает с `[F]` RW-38) | `[M]` |
| D-04 | `ui8/23-settings-categories.json` | модалка `{325,73,1398×961}`, 63 строки, `rootCls=container_abd9a8` | `[M]` |
| D-05 | `ui8/20a-searchbar.json` | поле поиска 244×28 / 242×26; contenteditable `editor__25bec` | `[M]` |
| D-06 | `ui8/23-settings-probe.json` | кнопки user bar на y=1072 (высота 32) → низ user bar 1104, верх 1048: user bar 56, кнопки 32 по центру | `[M]` |
| D-07 | `ui8/20i-scope-compare.json`, `20h-winner.json`, `20f-search-network.json` | рабочий ввод поиска (`ctrlA+typeChars`), эндпоинт `/api/v9/guilds/{id}/messages/search`, `offset=0` — поведение, не геометрия | `[M]` |

## 3. Токены (дамп CSS-переменных)

| id | Путь | Что взято | Статус |
|---|---|---|---|
| T-01 | `ui/tokens.json` | 4795 переменных; `--custom-guild-list-width=calc(40px+12px*2)` (=64), `--custom-channel-header-height=49px`, `--custom-channel-textarea-text-area-height=56px`, `--custom-member-list-width=256px`, `--guildbar-avatar-size=40px`, `--chat-markup-line-height=1.375rem` | `[T]` |
| T-02 | `ui/tokens.json` | цвета: `--brand-500` = `#5865f2`, `--brand-560` = `#4654c0`, `--status-danger`/`--red-400` = `#da3e44`, `--status-warning`/`--yellow-300` = `#fdb833`, `--status-positive`/`--green-360` = `#3d9e60`, `--green-new-38` = `#45a366`, `--text-muted` = `#96979e`, `--text-link` = `#4d96ee` | `[T]` |
| T-03 | `ui/tokens.json` | `--custom-layout-sidebar-width=232px`, `--modal-width-{small,medium,large}=442/602/800px`, `--custom-emoji-picker-constants-{min-emoji-picker-width,emoji-picker-height,unicode-category-shortcut-height}=498/440/48px`, `--menu-scroller-block-padding=8px` | `[T]` |

Полная сверка `tokens.css` ↔ дамп: `node waves/RW-39/PM/w39a/verify-tokens.mjs` — **205 токенов**
(после D-02/D-04), 57 ссылок `dump_refs`, из них **57 совпало, 0 расхождений, 0 отсутствующих**.
История: в итерации 1 было 196 токенов и 16 расхождений; по решению PM (D-02 «замер/дамп >
наше значение») значения приведены к дампу владельцем `tokens.css` (W39b2), `tokens.json`
перегенерирован. Расхождения итерации 1 — в `PM/w39a/REPORT.md` §4.1; `tokens.css` правился
только владельцем (W39a его не меняет).

## 4. RE-контракт RW-37 (принят, не переизмерялся)

| id | Путь | Что взято | Статус |
|---|---|---|---|
| L-01 | `waves/RW-37/RE/layout.json` | `rail_w=64`, `channel_tree_w=311`, `channel_tree_w_default=203`, `chat_w=1417`, `member_list_w=256`, `topbar_h=32`, `header_h=49`, `composer_h=56`, `member_avatar=32`, `message_avatar=40`, `message_indent_cozy=64`, `line_height=22`, `row_heights={message_cozy:26, member:42, channel:28}`, `row_heights_detail.message_cozy_group_start=48`, `member_window.rows=100`, `member_count_alt=24229`, `online_count=663` | `[L]` |

Сверка с пикселями (R-01, R-08): `topbar 32 ✓`, `header 49 ✓`, `tree 311.2 ✓`,
`member_list 256 ✓`, `composer 56 ✓`, `channel 28 / шаг 30 ⚠` (в RW-37 только высота строки),
`member 42 ✓ (43.2 по JPEG, ±1)`.

## 5. Компонентные контракты (RE-док)

| id | Путь | Что взято | Статус |
|---|---|---|---|
| C-01 | `docs/re/frontend-d03.md` §1.1–1.4 | дерево поиска: `searchResultsWrap`, 6 items на срез, фильтры/сортировка/сеть; семантика чипа области | `[M]` (тот же CDP-снимок) |
| C-02 | `docs/re/frontend-d03.md` §2 | пикер эмодзи: 10 категорий, 42/50 ячеек, searchPh, skin-tone | `[M]` |
| C-03 | `docs/re/frontend-d03.md` §3 | участники: `member__5d473`, `membersGroup*`, окно 100 строк | `[M]` |
| C-04 | `docs/re/frontend-d03.md` §4 | настройки: `container_abd9a8`, 38 переключателей, категории | `[M]` |

## 6. Fixtures

| id | Путь | Что взято | Статус |
|---|---|---|---|
| F-01 | `fixtures/MANIFEST.json` | курированные конформанс-фикстуры (gateway/ready-stats, rest и т. д.) — источник **данных** для экранов (24 229 строк участников и т. п.), не геометрии | `[L]` (манифест) |
| F-02 | `fixtures/gateway/summary.json`, `fixtures/rest/*` | используются W39e/W39d для наполнения списков; размеры строк берутся из `[F]`/`[L]`, не из фикстур | `[L]` |

## 7. Чего нет в дампе (и как обойдено)

| Пробел | Факт из дампа | Влияние / обход |
|---|---|---|
| Меню гильдий | `log.json` `guild-menu items: 0`; `07-guild-menu.html` = 41 байт; PNG почти пустой | попап гильдий в Phase 2 не верстаем; `SPEC §7.14` даёт только общий popover `[A]` |
| Список ЛС | `05-dm-channels.json` = `[]`, `dm-list channels: 0` | экран ЛС вне Phase 2; из R-05 взят только каркас |
| Результаты поиска (данные) | `04b-search-results.json` = 2 байта (`{}`) | фильтры/сортировка разобраны по DOM (R-10/R-11), сами результаты — только PNG; в спайке поиск локальный `[F]` |
| Профиль-попаут: текст | `06-profile-text.json` `{"text": null}`, `hasText: false` | размеры попаута есть (R-06), содержимое — из фикстур W39e |
| Контекстное меню сообщения: геометрия | `01-msg-menu-items.json` — только 15 текстов, без rect | высота пункта меню 32 `[A]` (`--row-menu-item`), не измерение |
| Квик-свитчер: геометрия | `03-quickswitcher-results.json` — 8 строк текста, без rect | экран не входит в Phase 2 |
| Настройки: остальные разделы | снят только «Голос» (`23-settings-voice.json`), есть `23-settings-error.jpg` | контент остальных разделов — из фикстур, `[F]` 38 переключателей |
| Гифки/стикеры | в дампе только вкладки (3 шт.), контент не снят | вне Phase 2 (`SPEC §9.2` `[A]`) |
| Hover/focus/active-состояния | статичные PNG не содержат | цвета взяты из токенов `[T]`/`[A]`, проверка — Phase 2 визуальной сверкой |
| Тултипы, скроллбар-трек | не сняты; есть только цвет thumb `#5f606a` `[T]` | радиус/паддинги тултипа `[A]` |
| Анимации/переходы | статика | длительности `[A]` (`--motion-*`) |
| Шрифты | `gg sans`, `ABC Ginto`, `Noto Sans` как веб-ассет — **намеренно не дампятся** | только `system-ui` (SPEC §4) |
| Иконки/спрайты/аватары/эмодзи-картинки | в репозиторий **не переносятся** | свои inline-SVG 24×24 и системный emoji (SPEC §12) |

## 8. Юридическая рамка (кратко)

1. `/home/sanya/re-deep` — golden, только чтение: ни один файл не изменялся, PNG/JPG не
   копировались в репозиторий.
2. Переносятся **значения**: цвета (`#5865f2`), размеры (`49px`), отступы, высоты строк.
   Это измерения, а не элементы фирменного стиля.
3. Бренд в UI — **`DL`**; названия «Discord», «Nitro», «Boost» и чужие логотипы не
   используются. Акцент `#5865f2` — значение токена `--brand-500`, а не логотип.
4. Шрифты — только системные (`system-ui` + системный emoji-стек); `@font-face` и `url()`
   запрещены. Иконки — собственные inline-SVG. Сторонние наборы не подключаются.
5. Сеть в UI отсутствует: экраны работают на `fixtures/` или локальных данных спайка.
