# W39h / P3-02 «чат-вью» — отчёт воркера

Дата: 2026-10-23. PM: DeepSeek. Статус: **green, готово к приёмке**.
Окно/probe/cargo/gate не запускались (WINDOW-BUDGET). Сети нет, `package.json` не менялся.

## 1. Итог

| Проверка | Команда | Результат |
|---|---|---|
| unit-набор (4 файла) | `node --test tests/unit/gui-spike-{dom,model,bench,features}.test.mjs` | **120/120, 0 fail, 0 skip** (было 102) |
| типы | `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit 0, 0 ошибок |
| геометрия | `node waves/RW-39/PM/w39c/geometry-check.mjs` | 31/31, exit 0 |
| red-first | `waves/RW-39/PM/w39h/red.log` | 31 pass / **18 fail** — ровно новые проверки |
| green | `waves/RW-39/PM/w39h/green.log` | 120/120 |
| мутации | `waves/RW-39/PM/w39h/mutations/*.log` | **6/6 caught** (0 missed) |

Новые тесты: **+18** (15 в `gui-spike-features.test.mjs`, 3 в `gui-spike-dom.test.mjs`).
Существующие 102 не менялись и не ослаблялись: `actions`-снимок, `2 ребёнка у .row`,
`sizer.children.length === rows`, `loadedRows ≤ 500`, `inFlight`, `console.error/warn` — все зелёные.

## 2. Фича → проверка → результат

| # | Фича (бриф/спека) | Проверка (тест) | Результат |
|---|---|---|---|
| 1 | group start/continuation (MSG-04/05) | `чат-вью…: group start/continuation…` — `data-group` 0/1, 2 ребёнка | ok |
| 2 | day divider (DIV-05…07) | `.day-divider[role=separator]`, `aria-label` с годом, подпись | ok |
| 3 | unread-полоса **34** (D-13, DIV-01…04) | `.chat-unread-divider[data-height=34]`, `role=separator`, «20+ новых», вне `#chat-list` | ok |
| 4 | reply-quote (MSG-19…22) | `.reply-quote` + `.avatar--xs` + ник + текст на строке 12 | ok |
| 5 | reactions-row (MSG-23…26) | пилюли `button.reaction[aria-pressed]` + `.reaction-count` | ok |
| 6 | embeds link/image (EMB-01…12) | `.embed--link` (провайдер/заголовок/описание), `.embed--image` → заглушка `data-state=placeholder` + `aria-hidden` + caption | ok |
| 7 | edited-метка (MSG-15) | `.edited` = «(изменено)» + суффикс в `aria-label` строки | ok |
| 8 | hover-панель R-16 | одна `.msg-toolbar[role=toolbar]`, `data-open` по `mouseover`, **15 пунктов** (4 реакции + 11 действий), у disabled — `title`, у активных — `data-command` | ok |
| 9 | «нет мёртвых кнопок» | реакция ставит `aria-pressed=true` в строке; reply включает чип ответа в композере | ok |
| 10 | композер: empty/typing/disabled (CMP-05…07) | `data-state`, `aria-disabled` без `disabled`-атрибута, причина в `data-disabled-reason` | ok |
| 11 | лимит 2000 + счётчик (CMP-06/13, K-10) | счётчик с 1800, `2000/2000`, ввод обрезается, `data-limit=exceeded`, `data-exceeded=true` | ok |
| 12 | attach/emoji affordance (CMP-09…12) | attach `aria-disabled` + `title`; emoji `aria-haspopup=dialog`; Ctrl+E открывает пикер, Esc возвращает чат, черновик цел | ok |
| 13 | 6 групп участников, блок **42** (MEM-02/04, D-13) | 6 `.member-group`, порядок роли→статусы, `.member-group-title[data-height=42]`, счётчик ≥ превью-строк, состав по `data-role`/`data-status` | ok |
| 14 | поиск по загруженному окну + счётчики (MEM-12/14) | `найдено N из M загруженных`; `N` сверен с **независимым** подсчётом по `store.pages`, `M` = `stats().loadedRows` | ok |
| 15 | экран участников: чипы групп, roving tabindex | 6 `.member-group-chip`, ровно один `tabindex=0`, ArrowRight двигает, клик → `aria-pressed` + выбранная строка группы (`aria-label` содержит статус) | ok |
| 16 | search overlay: 7 секций (R-10) | 7 `.search-section` с точными заголовками, 5 `input`, 1 `select`, 1 `switch`, **9** `.search-option` | ok |
| 17 | сортировка: 3 пункта (R-11) | `role=menu` + 3 `menuitemradio`, один `aria-checked`, `aria-expanded` триггера, «Самое старое» переворачивает выдачу | ok |
| 18 | результаты по загруженному окну, честно | `.search-result` ≤ 6 (срез), статус `найдено N из M загруженных (всего 100 000)`, «Показать ещё» | ok |
| 19 | переход к сообщению (SRCH-16) | клик по результату: оверлей закрыт, `scrollToIndex`, строка `.is-highlight` | ok |
| 20 | ошибки → `UIError` | невалидная дата: `UIError` в `.search-status[data-state=error]`, оверлей жив, сброс снимает ошибку | ok |
| 21 | a11y | roving tabindex (чипы + меню), `aria-selected/current/expanded/pressed`, focus-trap верхних слоёв (emoji/settings), меню сортировки — popover с закрытием по Tab/Esc | ok |
| 22 | инварианты списка | extras внутри `.text` (у `.row` ровно 2 ребёнка), `.text-body` первый и начинается с `#<index>`, `#chat-list.children.length === 1`, sizer = окно | ok |
| 23 | бюджет окна | после поиска/фильтров/переходов: `loadedRows ≤ 500`, `inFlight.size === 0` (чат и участники) | ok |
| 24 | D-13 из токенов | `--unread-banner-h: 34px`, `--row-member-group: 42px`; CSS использует `var(...)`, литералов нет | ok |

## 3. Что изменено

Новые модули (`rust/gui-spike/ui/features/`):
- `rows.js` — детерминированные extras строки, day divider, 9 опций «Содержит», 15 пунктов R-16;
- `reactions.js` — базовые реакции + локальные переключения (живут вне DOM, переживают скролл);
- `embeds.js` — link/image карточки, заглушка картинки без сети;
- `composer.js` — состояния empty/typing/disabled, лимит 2000, счётчик, reply-чип;
- `message-toolbar.js` — одна hover-панель на чат (переезжает по строке, не +15 узлов на каждую);
- `search-overlay.js` — 7 секций, меню сортировки, выдача срезом, честный статус, `UIError`.

Изменены: `app.js` (сборка + `currentApp()`, аддитивные поля snapshot), `features/chat.js`
(extras/unread/панель/jump), `features/members.js` (6 групп, чипы, счётчики), `features/search.js`
(`formatFound`, `onResult`), `features/actions.js` (Escape-приоритеты, `aria-haspopup` эмодзи);
CSS-дополнения: `components/chat.css`, `members.css`, `composer.css` (все значения — токены,
`url()`/`@font-face` нет). READ-ONLY не тронуты: `index.html` `3dcf9bc4…`, `model.js` `642e1d8f…`,
`tokens.css` не менялся, `src/**`, `DESIGN/**` не тронуты.

## 4. Мутации (тест ловит регресс)

| # | Мутация | Результат |
|---|---|---|
| M1 | убрать рендер reply-quote | **caught** (features: group/extras) |
| M2 | unread 34 → 44 | **caught** (features: unread-полоса) |
| M3 | блок группы 42 → 24 | **caught** (features: панель 6 групп) |
| M4 | лимит 2000 → 2100 | **caught** (features: композер) |
| M5 | hover-панель 15 → 14 | **caught** (features: R-16) |
| M6 | 6 групп → 5 | **caught** (features: панель 6 групп) |

Логи: `waves/RW-39/PM/w39h/mutations/M*.log`.

## 5. md5 (сдаваемые файлы)

```
eeb2494820791dc42f3a6bce555753b3  ui/app.js
0475d9673a607fe90006b66898bff459  ui/features/chat.js
08abe62134913f465b92f5b3809ce1e4  ui/features/members.js
6b6bb3168274af407f1874b7afecd261  ui/features/search.js
bc96a977d7f5193e6f7f26cb800d4ae4  ui/features/actions.js
77b01deca85d55eae27ae3ce5164aa01  ui/features/rows.js
13afe402bda036ca76db1e40f04055ca  ui/features/embeds.js
836a6a83de80f9b2cb6e464e9367d44c  ui/features/reactions.js
40ed7a3ab3ee66caebb1a9c9d3a76b5f  ui/features/composer.js
5953abc537c3980de49f50079ed4f7cb  ui/features/message-toolbar.js
72bc7e1f0044a13b546d42e844ae3f50  ui/features/search-overlay.js
cdc501a881545f263dfc904b14d7c636  ui/components/chat.css
18ba02980e3a9e8fb263afb6b56b20a7  ui/components/members.css
d6cd33d65791407bdee919325b547af6  ui/components/composer.css
736aaf03f095adac6c7210e1332b6833  tests/unit/gui-spike-features.test.mjs
25e8aaca4e4fd4bd8274a82f7e2b511f  tests/unit/gui-spike-dom.test.mjs
```

## 6. Остаток и честные ограничения

1. **Высота строк с extras.** Модель высот D-04 заморожена (26/48/+22, оракул RW-37), поэтому
   extras не увеличивают строку: индексы выбраны там, где запас есть (цитата — 48, реакции — 48,
   link-embed — 70, image-embed — 92, day divider — 48). Живым окном вёрстка **не проверялась**
   (WINDOW-BUDGET). Если PM хочет «настоящую» высоту с extras — это ревизия модели (карточка
   W39d-2/PM), не ослабление тестов.
2. **Unread-полоса — sibling списка**, а не inline-разделитель как в Discord: §6.1 запрещает
   `#chat-list` второго ребёнка, а inline требует правки `VirtualList`/модели высот. Сделана
   «перейти к непрочитанному» полосой сверху (`order:-1`), 34 border-box.
3. **Hover-панель одна на чат** и позиционируется по `top` строки − `scrollTop` (иначе 15 кнопок ×
   ~40 строк на каждый кадр скролла). MSG-29 говорит «внутри строки» — отличие осознанное,
   задокументировано в шапке `message-toolbar.js`.
4. **disabled композера** пока только программный (`composer.setDisabled(true, reason)`), реального
   триггера прав канала в спайке нет — ждёт контракта W39i (permission/rate limit).
5. **Вставка эмодзи в позицию курсора (CMP-12)** не реализована: требует правки `features/emoji.js`,
   которого нет в файловой матрице W39h. Affordance (Ctrl+E, `aria-haspopup=dialog`) закрыт.
6. **Search overlay — оконный поиск**: полный каталог за сервером (Phase 3). Секция «Содержит»:
   файл/звук/видео/опрос всегда 0 — в демо-данных их нет (честный `false`, не «всё»).
   «Самое актуальное» = порядок по индексу (в данных нет поля активности).
7. **Счётчики групп в панели** — по загруженному окну (≤50 строк, 8 превью на группу): в заголовке
   `АДМИНИСТРАЦИЯ — 17` (окно), в `aria-label` — «17 в загруженном окне»; превью-строк 8.
8. **DOM-шим** не даёт фокус/`closest`/всплытие: focus-trap панели проверяется структурно
   (roving tabindex, `aria-*`), реальный Tab-цикл — существующий для emoji/settings.
9. `data-height`/`data-count` дублируют числа токенов/D-13 атрибутами (CSS-каскад в шиме не
   считается); при смене `--unread-banner-h`/`--row-member-group` тест DOM сверяет `tokens.css`.
10. Экспорт `currentApp()` в `app.js` — диагностический API для поведенческих тестов (шим не умеет
    hover/focus); в проде не используется, IPC-контракт не менялся.
11. `pinned`-флаг (`index % 523 === 0`) и демо-дата 23.10.2026 — синтетика для секций фильтра;
    источник данных — только локальные страницы `PageStore`, сети нет.
