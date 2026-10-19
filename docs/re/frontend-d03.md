# D-03 — Экраны: поиск, эмодзи, участники, настройки + контракты компонент

- Дата: 2026-10-19. Среда: живой Discord (Chrome 148 / Electron 42, сборка 1.0.162), CDP `http://127.0.0.1:9223`.
- Метод: собственный CDP-драйвер (HTTP `/json/list` c `Connection: close` → WebSocket; `Runtime/Page/Input`), клики по `getBoundingClientRect`, скриншоты jpeg с `clip`.
- Ограничения соблюдены: сообщения не отправлялись, в голос не входили, значения настроек не менялись; `logs/`, `functions/`, `evidence/` не изменялись.
- Все новые артефакты: `/home/sanya/re-deep/ui8/` (драйверы `ui-search4..9.mjs`, `ui-emoji5..9.mjs`, `ui-settings7..13.mjs`, `ui-members6.mjs`).
- `design-tokens.json` в репозитории отсутствует → токены берём из снятых CSS-переменных живого клиента `ui/tokens.json` (4795 переменных, тема dark).

## 1. Поиск

### 1.1 Результаты (гильдийный поиск)
- Рабочий приём ввода: `Ctrl+A` в поле (снимает чип области) + посимвольный ввод. Поле — contenteditable `editor__25bec`, панель результатов `searchResultsWrap_a98f3b`, заголовок — `totalResults`.
- Реальные результаты: `ProBot` → «3 302 результата»; `Привет` → «49 887 результатов» (заголовок в панели).
- Структура группы/результата: `searchResultGroup_a7e67f` (aria «канал, категория») → `container__80bf8` / `searchResult__80bf8`; строка сообщения — `wrapper_c19a55 cozy_c19a55 zalgo_c19a55`, `id="search-result-<id>"`, иконка `<svg aria-label="Текст">`.
- В срезе DOM зафиксировано 6 items (`itemCount=6`), классы: `searchResultGroup_a7e67f`, `container__80bf8`, `searchResult__80bf8`, `message__80bf8`, `wrapper_c19a55`.
- Артефакты: `20-search-results.{json,html,jpg}`, `20a-searchbar.json`, `20a-search-open.jpg`, `20a-search-typed.jpg`, `20e-search-typed-rules.jpg`, `20h-winner.json` (победивший сценарий `ctrlA+typeChars`, эндпоинт `/api/v9/guilds/<gid>/messages/search`).

### 1.2 Фильтры
- Секции `filterSection_e4772e`: **От**, **В**, **Содержит**, **Упоминания**, **Дата** (+ кнопка «Добавить дату»), **Тип автора**, **Закреплено** (7 секций).
- Поля: `input__75098 comboBoxInput__0edde` с плейсхолдерами «Например, pharos12510», «Любой контент», «Например, «бот»», «Закреплено, Например, «Да»».
- Кнопки: «Очистить фильтры», «Отмена», «Применить фильтры».
- Опции «Содержит» (9): изображение, видео, ссылку, файл, вложение, аудио, опрос, стикер, переслать.
- Артефакты: `20b-search-filters.{json,html,jpg}`, `20b-search-filters-has.json`, `20d-scope-menu.{jpg,json}` (меню области поиска).

### 1.3 Сортировка
- Меню `menu_c1e9c4 flexible_c1e9c4`: «По новизне» (выбрано, `aria-checked`), «Самое старое», «Самое актуальное» (3 опции).
- Артефакты: `20c-search-sort.{json,jpg}`.

### 1.4 Сеть (важный нюанс)
- С чипом области: `GET /api/v9/guilds/1014537851418980372/messages/search?channel_id=1014537851867775060&content=ProBot&…` → `{"messages":[],"total_results":0}`.
- Без чипа: `…messages/search?content=ProBot&…` → `total_results=3302`; `content=Привет` → 49887.
- Ранее наблюдалась склейка `content=ProBot#📲┃основной` (тоже 0). Вывод: с `channel_id` канала `#📲┃основной` результаты пустые — особенность сборки, не ошибка драйвера (проверено сетью напрямую).
- Артефакты: `20f-search-network.json`, `20g-search-probe.json`, `20i-scope-compare.json`, `log-search4.json`, `log-search6.json`.

## 2. Эмодзи

### 2.1 Полный пикер (канал `#поговорить-тут`, открытие `Ctrl+E`)
- Контейнер: `positionLayer__08434 expressionPickerPositionLayer__74017 theme-dark images-dark layer__59d0d`.
- Вкладки `[role=tab]`: «Гифки», «Стикеры», «Эмодзи» (`aria-selected=true`) — 3 шт.
- Поиск: `input` с плейсхолдером «Найдите идеальный эмодзи»; кнопка «Добавить эмодзи».
- Секции (видимые): «Лучшие эмодзи сервера ITSKIDON PC BOOST», «Часто используемые».
- Сетка: 50 `emojiItem_fc7141` видимых, 17 `lockedEmoji_d982c2` (Nitro/заблокированные).
- Рейл категорий (10, скролл не потребовался, `scrollHeight≈clientHeight`): `top guild emoji`, `recent`, `ITSKIDON PC BOOST`, `people`, `nature`, `food`, `activity`, `travel`, `objects`, `symbols`. Категории сервера — `categoryItemGuildCategory_b9ee0c`, элементы — `categoryItem_b9ee0c`/`categoryPill_b9ee0c`; цвет кожи — aria «Выбрать цвет кожи эмодзи (Пусто)».
- Поиск «:fire» → 10 результатов, секция «Доступно с подпиской Nitro» (результаты — платные эмодзи; `aria-label` у элементов не заполнен — см. «Неизвестные»).
- Артефакты: `21-emoji-picker.{json,html,jpg}`, `21-emoji-search.{json,jpg}`, `21d-emoji-categories.{json,jpg}`, `log-emoji9.json`.

### 2.2 Полный пикер в ЛС (повтор для сравнения)
- `expressionPickerPositionLayer__74017` / `layer__59d0d`, поиск `input__75098`; рейка `navList__08434` (aria «Категории меню выбора реакции»); 42 эмодзи в сетке; ввод «:fire» → 11 результатов.
- Артефакты: `21b-emoji-full.{json,html,jpg}`, `21c-emoji-full-search.{json,jpg}`, `log-emoji7.json`.

### 2.3 Вариант «пикер реакций» (из контекстного меню сообщения)
- Пункт контекстного меню `message-add-reaction` → «тонкий» пикер: `emojiPicker_c0e32c` + `bodyWrapper_c0e32c`, `emojiPickerListWrapper_c0e32c`, `listWrapper_c656ac`, `categorySection_c656ac`, 5 рядов `emojiListRow_fc7141`, 60 `emojiItem_fc7141`, 43 `lockedEmoji_d982c2`. Поиска и вкладок нет.
- Артефакты: `21-emoji-picker.*` ранней серии (`log-emoji6.json`), см. также `21b-*`.

## 3. Участники (правый сайдбар)

- Контейнер: `aside[class*=membersWrap]` → `membersWrap_c8ffbb hiddenMembers_c8ffbb`.
- Группы: «Server Booster — 2», «В сети — 735» (элементы `membersGroup`); DOM отдаёт вложенные дубли, реальных групп 2.
- Строки: 35 видимых `member__5d473 member_c8ffbb container__91a9d clickable__91a9d`; внутри `memberInner__5d473 layout__91a9d`, аватар `wrapper__44b0c avatar__44b0c avatarStack__44b0c`, имя `name__91a9d text-md/medium__91a9d`, `subText__91a9d` (активность), tag `username__5d473`.
- Поля поиска по участникам в этом сайдбаре нет (`search: []`).
- Артефакты: `22-members.json` (JSon со срезом имён/токенов), `22-members.jpg`, `log-members6.json`.

## 4. Настройки

### 4.1 Открытие и категории
- Открытие: кнопка с `aria-label="Настройки пользователя"` в панели (шестерня). `Ctrl+,` в этой сборке нажатие не отработало (см. «Неизвестные»).
- Корень: `container_abd9a8` (1398×961). Категории — 63 строки в навигации (+поле «Поиск»):
  - **Учётная запись** (подпункты: Информация об учётной записи, Пароль и безопасность, Статус учётной записи, Семейный центр)
  - Данные и конфиденциальность · Разрешения для сообщений · Уведомления
  - **Nitro** · Буст сервера · Подписки · Склад подарков · Выставление счетов
  - **Голос и видео** · Внешний вид · Специальные возможности · Система · Язык и время
  - Зарегистрированные игры · Конфиденциальность активности · Подключённые приложения · Разработчик · Выйти
- Артефакты: `23-settings-categories.json`, `23-settings-open.jpg`, `23-settings-open2.jpg`, `23-settings-probe.json`, `23-settings-error.jpg`, `log-settings7..10.json`.

### 4.2 Вкладка «Голос и видео» (значения не менялись; расширенные блоки раскрыты)
- Разделы: **Голос** (Микрофон/Динамик + проверка), **Профиль ввода**, **Настройки чувствительности ввода**, **Шумоподавление**, **Эхоподавление/Режим рации/АРУ**, **Камера**, **Стриминг**, **Звуки**, **Связанные настройки**, **Расширенные**.
- Селекты (combobox): Микрофон «Система: Easy Effects Source»; Динамик «Система: Easy Effects Sink»; Камера «Нет видеоустройств»; **Шумоподавление → «Krisp»** (`aria-label="Шумоподавление"`, подпись «Предоставляется Krisp»).
- **Профиль ввода** — радиогруппа (`data-mana-component="BaseRadioGroup"`, `role=radiogroup`): «Изоляция голоса», «Студия», «Пользовательский».
- Переключатели: 38 в DOM (`<input role="switch" type="checkbox" data-react-aria-pressable>`); в срезе HTML состояния: Автоматически определять чувствительность ввода = вкл, Эхоподавление = вкл, Режим рации = **выкл**, Автоматическая регулировка усиления = вкл, Расширенная активация по голосу = вкл, Предложение аудиоустройств = вкл, Предупреждение об отсутствии звука = вкл, Предупреждение о переключении голосового канала = вкл, Качество обслуживания = **выкл**, Предпросмотр видео (всегда) = вкл, Показывать предпросмотр стрима = вкл, Приглушение стрима = вкл, «Откл. звук/Включить звук в динамиках/Заглушить/Вкл. микрофон» = вкл и др. (звуки звуковой панели).
- Ползунки (`role=slider`, класс `slider__4c059`): Громкость микрофона 0–100 (100); Громкость динамика 0–200 (100); Сила приглушения стрима 1–100 (50, подпись 49%); 4-й ползунок 0–100 (≈26.11; подпись вне среза HTML — вероятно порог чувствительности ввода).
- Кнопки: «Проверка микрофона»/«Прекратить проверку», «Проверить видео», «Загрузить журналы», «Показать папку», «Сброс».
- Артефакты: `23-settings-voice.{json,html,jpg}`, `23-settings-voice-expanded.jpg`, `log-settings11..13.json`.

## 5. Контракты компонент (12)

Формат: **компонент** — где в JS (модуль/файл), варианты, размеры, состояния, токены, цитата (`файл:смещение`, ≤5 строк).

1. **Button** (`data-mana-component="button"`)
   - Компонент: модуль 657718 @ `discord.com_assets_web.4d4b2f75fdea5470.js`:
     ```
     function S(e){let{role:t="button",variant:n="primary",size:a="md",text:_,icon:A,iconPosition:h="start",fullWidth:g=!1,focusProps:S,loading:R,rounded:D=!1,type:v="button",disabled:F,minWidth:B,...}
     ```
   - CSS: модуль 65406 @ `discord.com_assets_384209.c316f250feb7e201.js:438` — `button:"button_a22cb0" … xs/sm/md, primary, secondary, "critical-primary", "critical-secondary", "overlay-primary", "overlay-secondary", "icon-only", "input-accessory", active, expressive, loading, spinner*`.
   - Варианты: primary, secondary, critical-primary, critical-secondary, overlay-primary, overlay-secondary, icon-only, input-accessory, active (toggle), expressive; отдельные: `text-button` (модуль 123292), `toggle-icon-button` (876798).
   - Размеры: xs, sm, md (+`spinner-lg`); состояния: hover/active, disabled, loading (`aria-busy` + fadeIn/fadeOut + spinner), hasText, fullWidth, rounded, minWidth→`--__button-min-width`.
   - Токены: `--button-border-radius` (`2147483647px` — пилюля), `--button-border-radius-lg`, `--button-danger-background-disabled`, `--button-outline-brand-border-active`, `--button-outline-primary-text`.
   - Живое: `button_a22cb0 sm_a22cb0 secondary_a22cb0 hasText_a22cb0` + `buttonChildrenWrapper_a22cb0`.
2. **TextInput / TextArea** (`input__75098`, `data-mana-component="text-area"`)
   - Модуль 103557 @ `discord.com_assets_05bfdf7077e1fc11.js:1161` — `…rows:j,disabled:A,"data-mana-component":"text-area",…onChange…` (autoFocus, minLength, maxLength).
   - CSS 358983 @ `discord.com_assets_348181.c6a72b8c6216ab6c.js:10813` — `input:"input__75098", hasLeading, leadingText, hasTrailing, hasTags, iconButton, clearButton`.
   - Варианты: однострочный/textarea, с чипами (фильтры поиска), с леад/трейл-иконками, комбо (`comboBoxInput__0edde`).
   - Токены: `--input-background-default`, `--input-icon-default`, `--input-placeholder-text-default`, `--input-border-error-default`, `--input-text-error-default`.
3. **Menu (Mana)**
   - CSS 798819 @ `discord.com_assets_261000.1b6532d3649c0f52.js:6116` — `menu:"menu_c1e9c4", scroller, scrollerWithScrollbar, fixed, item, label, caret, disabled, separator:"separator_c1e9c4", submenu, submenuPaddingContainer, colorDefault…`; реализация меню — модуль 980707 (@ web-бандл).
   - Варианты пунктов: colorDefault/colorDanger, checkbox, radioSelection, submenu; разделитель — `separator_c1e9c4`.
   - Токены: `--menu-scroller-block-padding: 8px`, `--menu-border-width: 1px`.
   - Живое: `menu_c1e9c4 flexible_c1e9c4` (меню сортировки, `aria-checked` на выбранном).
4. **ContextMenu**
   - Опенер: модуль 837528 (`(0,a.L3)`); примитивы `L3/Z_/jA` — модуль 442433 (`CONTEXT_MENU_OPEN/CLOSE`); клавиатурный реестр — модуль 696760 (`mod+f`→`FOCUS_SEARCH`), `mod+k` — 675704 (QuickSwitcher).
   - Стаб без пунктов — модуль 477782 (`return null`).
   - Живое: пункт `message-add-reaction` («Добавить реакцию») открывает пикер реакций; элемент меню — id-хеш вида `message-*`.
   - Рендер — тот же Mana-Menu (см. п.3), в поповер-слое.
5. **Modal** (`data-mana-component="layer-modal"`)
   - Модуль 521489 @ `discord.com_assets_e6c31aeaf72a699a.js` — `role:"dialog"`, `aria-label/aria-labelledby:heading-<id>`, `transitionState`, `animationVariant:"default"`, `onClose`, `impressionType: MODAL`; фокус-ловушка — модуль 617986 (`[data-mana-component="modal"], [role="dialog"][tabindex="-1"]`).
   - Токены: `--modal-background`, `--modal-footer-background`, `--modal-vertical-padding: 16px`, `--modal-width-small: 442px`, `--modal-width-medium: 602px`, `--modal-width-large: 800px`.
   - Живое: окно настроек — `container_abd9a8` (собственная оболочка настроек, не Mana-Modal).
6. **Popover** (`data-mana-component="popover"`)
   - Модули: 179657 @ `discord.com_assets_267e963ebabea6ad.js:9316`; 43105 @ `460797267c8bdd59.js:860`; 273875 @ `49b5f33b11f4e584.js:3991` — поповеры меню/статусов/шорткатов.
   - Живое: поповеры поиска/эмодзи/реакций живут в `layer__59d0d`/`positionLayer__*` (см. §1–2).
7. **Tooltip**
   - CSS 233680 @ `discord.com_assets_348181.c6a72b8c6216ab6c.js:657` — `YL:"tooltip_c36707", Cy:"tooltipDisablePointerEvents_c36707", xQ:"tooltipPointer_c36707", r…` (+`tooltipContent`, `tooltipContentAllowOverflow`).
   - Вариант `mini-premium-tooltip` — модуль 461536 @ `discord.com_assets_bb94c0b7508a1297.js:3161` (награды/бусты).
   - Живое (пикер): `richTooltipContent__9038b`, `tooltipPremiumFooterSegment__7d7fe`, `tooltipBodyContainer__7d7fe`.
8. **Switch**
   - React-aria: модуль 35556 @ `discord.com_assets_501428.98d57a9bd8cbb604.js:1181154` — `inputProps:{...o,role:"switch",checked:a}`.
   - Использование: модуль 479909 @ `discord.com_assets_838aef4965d49a40.js:100367` — `{role:"switch","aria-checked":c,onClick:…}`.
   - Живое: `<input type="checkbox" role="switch" data-react-aria-pressable checked>` (38 шт. на вкладке «Голос и видео»).
   - Токены: `--switch-background-selected-default`, `--switch-border-default`, `--switch-border-selected-hover`, `--switch-thumb-background-default`, `--switch-thumb-icon-active`.
9. **Slider**
   - CSS 9761 @ `discord.com_assets_500186.e89df1f0566f54ee.js:130` — `slider:"slider__4c059", mini, bar, grabber, disabled, mark*, defaultValue, hasMarks`.
   - Живое: `<div role="slider" class="slider__4c059" aria-valuemin/max/now aria-orientation="horizontal" aria-labelledby>`.
   - Токены: `--slider-track-background`.
10. **Avatar**
    - CSS 311582 @ `discord.com_assets_113561.4511bb7966772f25.js:259` — `{m:"avatar_fdd9da",X:"grayscaleAvatar_fdd9da"}`; рамка/статус — модуль 818888 @ `120561.03196bb644de78e5.js:123` — `{R:"avatarWrapper_d8a370",h:"ring_d8a370"}`.
    - Живое: `wrapper__44b0c avatar__44b0c avatarStack__44b0c` (стек из 34 в участниках).
11. **Badge**
    - Модуль 306091 @ `discord.com_assets_880248.92a937f61f4f886c.js:299` — `numberBadge__463b7 base__463b7`, `iconBadge__463b7`, `premiumBadge__463b7`, `baseShapeRound*`.
    - Другие контуры: модуль 198476 (`numberBadge_c7a32c`), модуль 672812 (`numberBadge__2ea32`), тег-бейдж сервера — 638855 (`badge_e7c728` @ `100212505f0d860a.js:357`).
    - Токены (22): `--badge-secondary-background`, `--badge-expressive-text`, `--badge-epic-border-gradient-end`, `--badge-mythic-background` и др.
12. **Scroller**
    - CSS 6095 @ `discord.com_assets_193433.213ada0bddfee7ee.js` — `scrollerBase_d125d2`, `auto/none/thin`, `scrollbarGutter`, `scrollbarGutterStable`, `customTheme`, `fade`, `scrolling`, `content`, `disableScrollAnchor`.
    - Виртуализованный список: `scroller_affa7e` + `listItems_affa7e` (рейл эмодзи, список участников); меню-скроллер: `scroller_c1e9c4`.
    - Токены: `--scrollbar-thin-thumb/track`, `--scrollbar-auto-thumb/track`.

## 6. Токены (фрагмент, `ui/tokens.json`, тема dark)

| Токен | Значение |
|---|---|
| `--modal-width-small/medium/large` | 442px / 602px / 800px |
| `--modal-background`, `--modal-footer-background` | hsl(240 6.494% 15.098%) |
| `--modal-vertical-padding` | 16px |
| `--switch-background-selected-default` | hsl(234.935 85.556% 64.706%) |
| `--switch-border-default` | hsl(240 4% 60.784% / 0.2) |
| `--switch-thumb-background-default` | hsl(0 0% 100%) |
| `--input-background-default` | hsl(0 0% 0% / 0.1216) |
| `--input-border-error-default` | hsl(1.905 90% 72.549%) |
| `--button-border-radius(-lg)` | 2147483647px (пилюля) |
| `--button-danger-background-disabled` | hsl(355.636 64.706% 50%) |
| `--slider-track-background` | (есть; значение — CSS-микс темы) |
| `--menu-scroller-block-padding` / `--menu-border-width` | 8px / 1px |
| `--scrollbar-thin-thumb` | hsl(234.545 5.473% 39.412%) |
| `--badge-secondary-background` | hsl(240 4% 60.784% / 0.1608) |
| `--text-strong` / `--background-surface-high` | hsl(0 0% 98.431%) / hsl(240 6.494% 15.098%) |

Отдельных `--tooltip-*`, `--avatar-*`, `--divider-*` токенов нет — компоненты используют общие (`--text-strong`, `--background-surface-high`, `--border-normal/strong`, `--interactive-background-hover`).

## 7. Неизвестные и ограничения

1. `aria-label` у эмодзи в результатах поиска «:fire» не заполнен (имена пустые) — в DOM только спрайты.
2. Подпись 4-го ползунка в «Голос и видео» не попала в срез HTML (aria-labelledby указывает на id вне среза); вероятно порог чувствительности.
3. Значения селекта «Шумоподавление» (кроме текущего «Krisp») не снимались — список бы не открывали, чтобы ничего не менять.
4. Пикер реакций — «тонкий» вариант (без поиска/вкладок): полный пикер доступен только у поля ввода сообщения.
5. Отдельного Mana-компонента Divider нет: разделители реализуются по месту (`separator_c1e9c4` в меню; `divider_c791b2`, `sectionDivider_e9cb00 divider_e9cb00` и т.п. в фичах).
6. Ctrl+, в сборке не открывает настройки через CDP-нажатие (открывали шестернёй «Настройки пользователя»); причина не установлена.
7. Чип области поиска (`channel_id`) в этой сборке стабильно даёт 0 результатов (проверено сетью) — особенность сборки 1.0.162.
8. Счётчик «В сети — 735» — онлайн на момент снятия; общее число участников сервера не снималось.

## 8. Артефакты (в `/home/sanya/re-deep/ui8/`)

| Группа | Файлы |
|---|---|
| Поиск | `20-search-results.{json,html,jpg}`, `20a-searchbar.json`, `20a-search-open.jpg`, `20a-search-typed.jpg`, `20b-search-filters.{json,html,jpg}`, `20b-search-filters-has.json`, `20c-search-sort.{json,jpg}`, `20d-scope-menu.{jpg,json}`, `20e-search-typed-rules.jpg`, `20f-search-network.json`, `20g-search-probe.json`, `20h-winner.json`, `20i-scope-compare.json` |
| Эмодзи | `21-emoji-picker.{json,html,jpg}`, `21-emoji-search.{json,jpg}`, `21b-emoji-full.{json,html,jpg}`, `21c-emoji-full-search.{json,jpg}`, `21d-emoji-categories.{json,jpg}` |
| Участники | `22-members.{json,jpg}` |
| Настройки | `23-settings-categories.json`, `23-settings-voice.{json,html,jpg}`, `23-settings-voice-expanded.jpg`, `23-settings-open.jpg`, `23-settings-open2.jpg`, `23-settings-probe.json`, `23-settings-error.jpg` |
| Логи/драйверы | `log-*.json`, `ui-*.mjs` (14 драйверов) |

Все скриншоты ужаты до ≤130 КБ (пережаты без потери структуры: `20a-*`, `20d`, `20e`, `21b`, `21c`, `21d`, `23-settings-open*`).
