# RW-39 / W39b — DOM-CONTRACT (обязателен для W39c/W39d)

Статус: заморожен до кода Phase 2. Дата: 2026-10-23. Автор: W39b.
Спецификация дизайна — `SPEC.md`; значения — `tokens.css`.

Любое расхождение с этим файлом — **сначала правка контракта** (с записью в отчёте и
согласованием PM), потом код. Классы/id/ARIA ниже нельзя переименовывать «по вкусу»:
часть из них — оракул тестов RW-38 (`[F]`), часть — требование ТЗ §4.

## 1. Что здесь зафиксировано

| Блок | Содержание |
|---|---|
| §2 | инварианты RW-38, которые нельзя ломать (список ассертов) |
| §3 | канонический `ui/index.html` (полное дерево) |
| §4 | таблица id / класс / роль / обязательные атрибуты |
| §5 | состояния: `data-*`, `.active`, `aria-*` |
| §6 | виртуализация: sizer/rows, высоты, окно, PageStore |
| §7 | шаблоны строк, которые генерирует W39d |
| §8 | события, `data-action`, клавиатура, IPC/`UI_SNAPSHOT` |
| §9 | совместимость с RW-38 и **требуемая правка харнесса** (блокер) |
| §10 | чек-листы W39c/W39d и приёмки |

## 2. Инварианты RW-38 (нельзя ломать) `[F]`

Источник: `tests/unit/gui-spike-dom.test.mjs` (читать как оракул, не менять).

> **Ревизия D-04 (W39d, 2026-10-23).** Высоты строк в пп. 4 и 6 заменены на
> переменную модель RE-оракула (`waves/RW-37/RE/layout.json`): сообщение
> 26 (продолжение) / 48 (начало группы) / +22 за перенос, участник 42. Остальные
> инварианты `[F]` не менялись. Ожидания теста обновлены парами «индекс → высота»
> и проверены мутацией «вернуть константу 44» — см. §9.5.

1. Ровно 4 `.screen` с id `screen-chat`, `screen-emoji`, `screen-members`,
   `screen-settings` — в этом порядке в `index.html`.
2. Ровно 4 `.rail-btn`, порядок `data-screen` = `chat`, `emoji`, `members`, `settings`.
   **`data-screen` разрешён только на `.rail-btn`** (и на кнопках в шиме): тест собирает
   regex-ом `/data-screen="([^"]+)"/g` все вхождения по `index.html` и сравнивает список с
   `.rail-btn`. Любой другой элемент с `data-screen` (например `<body>`) ломает сравнение —
   для активного экрана на body используется `data-active-screen`.
   После клика — ровно один `.rail-btn.active` и ровно один `.screen.active`.
3. `.screen-title` — один элемент; тексты **дословно**:
   `Чат · 100 000 сообщений`, `Эмодзи-пикер · 10 категорий`, `Участники · 24 229`,
   `Настройки · 38 переключателей`.
4. `#chat-list`: ровно **один** ребёнок — `div` sizer (`style.position="relative"`,
   `height = top(100000)` — сумма высот строк). Строки `.row` лежат **внутри
   sizer**, у них `style.height` = 26 (продолжение) / 48 (начало группы) + 22 за
   каждый перенос (D-04), `style.top = top(index)` (префиксная сумма тех же
   высот), `data-state="row"`.
5. Строка чата: ровно 2 элемента-ребёнка — `.author` и `.text` (по одному каждого).
   Placeholder (`data-state="placeholder"`) — **0 детей**.
6. `#member-list`: sizer `height = 24229 × 42px`; `.member` имеет `style.height="42px"`,
   `data-state="row"`, ровно 3 ребёнка; текст `children[1]` начинается с `member_<i>_`.
7. `#emoji-tabs`: ровно 10 `.tab`; ровно один `.tab.active` и `aria-selected="true"`;
   `#emoji-grid .emoji-cell` ≤ 120, без дублей, `cell.title === cell.textContent`.
8. `#settings`: ровно 38 `.toggle`, ровно 38 `input[type="checkbox"]` (все `checked`),
   ровно 3 `h3` с текстами `Голос и видео`, `Интерфейс`, `Уведомления`.
9. `#bench`, `#m-fps`, `#m-p95`, `#m-rss`, `#m-threads` существуют и обновляются.
10. `UI_SNAPSHOT`: `screenCount=4`, `chatRows=100000`, `memberRows=24229`,
    `emojiCategories=10`, `settingsToggles=38`, `chatRenderedRows≥10`,
    `memberRenderedRows≥10`, `loadedRows ≤ 500` и кратно 100,
    `actions = {screenClicks:4, categoryClicks:true, toggleClick:true, listScroll:true}`.
11. В обычной работе нет `console.error`/`console.warn`; DOM-строки переиспользуются,
    sizer не копит отвязанные узлы (`sizer.children.length === rows.length`).

Следствие: **новые узлы можно добавлять только вокруг** перечисленных, не меняя их
родителя-«списка», число детей и высоты строк.

## 3. Канонический `ui/index.html`

Обязательная структура (W39c копирует как есть, добавляя только контент, который генерирует
W39d). Комментарии `<!-- [F] -->` — трогать нельзя; `<!-- gen -->` — заполняется из JS.

```html
<!doctype html>
<html lang="ru" data-density="cozy">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="color-scheme" content="dark" />
    <title>DL — discord-lab (P3-01)</title>
    <link rel="stylesheet" href="components/tokens.css" />
    <link rel="stylesheet" href="components/primitives.css" />
    <link rel="stylesheet" href="style.css" />
    <link rel="stylesheet" href="components/rail.css" />
    <link rel="stylesheet" href="components/tree.css" />
    <link rel="stylesheet" href="components/chat.css" />
    <link rel="stylesheet" href="components/emoji.css" />
    <link rel="stylesheet" href="components/members.css" />
    <link rel="stylesheet" href="components/settings.css" />
  </head>
  <body data-active-screen="chat">
    <!-- 1. Строка заголовка: 32px, на всю ширину (RE topbar_h=32). -->
    <header class="topbar" role="banner">
      <div class="screen-title" role="heading" aria-level="2" aria-live="polite">Чат · 100 000 сообщений</div>
      <div class="metrics" aria-live="polite">
        <span>FPS <b id="m-fps">—</b></span>
        <span>p95 кадра <b id="m-p95">—</b></span>
        <span>RAM <b id="m-rss">—</b></span>
        <span>потоки <b id="m-threads">—</b></span>
        <button id="bench" class="bench" type="button">Замер 5 с</button>
      </div>
    </header>

    <div class="shell">
      <!-- 2. Рейл 64px: бренд DL, гильдии, разделитель, 4 кнопки экранов. -->
      <aside id="guild-rail" class="rail" role="navigation" aria-label="Серверы и экраны">
        <div class="brand" title="discord-lab" aria-hidden="true">DL</div>
        <button class="guild-item is-selected" type="button" data-guild="1" aria-label="Сервер ITSKIDON PC BOOST" aria-current="true">IS</button>
        <button class="guild-item" type="button" data-guild="2" aria-label="Сервер 2">S2</button>
        <button class="guild-item" type="button" data-guild="3" aria-label="Сервер 3">S3</button>
        <div class="rail-sep" role="separator" aria-hidden="true"></div>
        <button class="rail-btn active" type="button" data-screen="chat" title="Чат 100k" aria-label="Чат" aria-current="page">💬</button>
        <button class="rail-btn" type="button" data-screen="emoji" title="Эмодзи-пикер" aria-label="Эмодзи">😀</button>
        <button class="rail-btn" type="button" data-screen="members" title="Участники 24k" aria-label="Участники">👥</button>
        <button class="rail-btn" type="button" data-screen="settings" title="Настройки" aria-label="Настройки">⚙️</button>
      </aside>

      <div class="workspace">
        <!-- 3. Дерево каналов 240px. -->
        <aside id="channel-tree" class="tree" role="navigation" aria-label="Каналы сервера">
          <header class="tree-header">
            <h2 class="tree-guild">ITSKIDON PC BOOST</h2>
            <button class="icon-btn" type="button" data-action="guild-menu" aria-label="Меню сервера" aria-haspopup="menu" aria-expanded="false">…</button>
          </header>
          <nav class="tree-scroll" aria-label="Список каналов">
            <section class="channel-group">
              <h3 class="channel-group-title">
                <button class="group-toggle" type="button" aria-expanded="true">НОВОСТИ</button>
              </h3>
              <ul class="channel-list" role="list">
                <li><button class="channel-item" type="button" aria-current="page"><span class="channel-icon" aria-hidden="true">#</span><span class="channel-name">новости</span></button></li>
                <li><button class="channel-item is-unread" type="button"><span class="channel-icon" aria-hidden="true">#</span><span class="channel-name">объявления</span></button></li>
              </ul>
            </section>
            <section class="channel-group">
              <h3 class="channel-group-title">
                <button class="group-toggle" type="button" aria-expanded="true">ГОЛОСОВЫЕ КАНАЛЫ</button>
              </h3>
              <ul class="channel-list" role="list">
                <li><button class="channel-item" type="button"><span class="channel-icon" aria-hidden="true">🔊</span><span class="channel-name">Пекарня</span></button></li>
              </ul>
            </section>
          </nav>
          <!-- 4. Строка пользователя 56px. -->
          <footer id="user-bar" class="user-bar">
            <button class="user-profile" type="button" data-action="profile" aria-label="Профиль: ShadowByte, невидимый">
              <span class="avatar avatar--sm" aria-hidden="true">SB</span>
              <span class="user-meta"><span class="user-name">ShadowByte</span><span class="user-status">Невидимый</span></span>
            </button>
            <div class="user-actions">
              <button class="icon-btn" type="button" data-action="mute" aria-label="Выключить микрофон" aria-pressed="false">🎙</button>
              <button class="icon-btn" type="button" data-action="deafen" aria-label="Выключить звук" aria-pressed="false">🎧</button>
              <button class="icon-btn" type="button" data-action="settings" aria-label="Настройки пользователя">⚙</button>
            </div>
          </footer>
        </aside>

        <main class="content">
          <!-- 5. Шапка чата 49px (только экран chat). -->
          <header id="chat-header" class="chat-header">
            <button class="icon-btn" type="button" data-action="toggle-tree" aria-label="Свернуть каналы" aria-expanded="true">☰</button>
            <h1 class="chat-title"><span class="hash" aria-hidden="true">#</span><span class="chat-title-text">новости</span></h1>
            <div class="chat-actions">
              <button class="icon-btn" type="button" data-action="threads" aria-label="Ветки">🧵</button>
              <button class="icon-btn" type="button" data-action="inbox" aria-label="Уведомления" aria-disabled="true" title="Уведомления (Phase 3)">🔔</button>
              <button class="icon-btn" type="button" data-action="pins" aria-label="Закреплённые сообщения">📌</button>
              <button class="icon-btn" type="button" data-action="members" aria-label="Участники">👥</button>
              <search class="search-field" role="search">
                <input id="chat-search" class="search-input" type="search" placeholder="Поиск" aria-label="Поиск сообщений" autocomplete="off" />
              </search>
            </div>
          </header>

          <!-- 6. Экран chat: чат + панель участников. -->
          <section id="screen-chat" class="screen active" role="region" aria-label="Чат" aria-hidden="false">
            <div class="chat-main">
              <div id="chat-list" class="list chat-list" tabindex="0" role="list" aria-label="Сообщения" aria-busy="false" data-row-height="44"></div>
              <p class="chat-notice" role="note">Отправка недоступна в спайке (без сети)</p>
              <div id="composer" class="composer" role="group" aria-label="Поле ввода сообщения">
                <button class="icon-btn composer-attach" type="button" data-action="attach" aria-label="Прикрепить файл">＋</button>
                <textarea id="composer-input" class="composer-input" rows="1" placeholder="Написать сообщение…" aria-label="Сообщение" aria-describedby="composer-hint"></textarea>
                <button class="icon-btn" type="button" data-action="emoji" aria-label="Эмодзи" aria-haspopup="dialog" aria-expanded="false">😀</button>
                <button class="icon-btn" type="button" data-action="gift" aria-label="Подарок">🎁</button>
              </div>
              <p id="composer-hint" class="visually-hidden">Отправка сообщений в спайке отключена</p>
            </div>
            <aside id="member-panel" class="member-panel" role="complementary" aria-label="Участники сервера">
              <h2 class="member-panel-title">Участники — <span id="member-online">735</span></h2>
              <div id="member-panel-list" class="member-panel-list" role="list" aria-label="Участники в сети"></div>
              <button class="member-panel-all" type="button" data-action="all-members">Показать всех</button>
            </aside>
          </section>

          <!-- 7. Экран emoji: карточка 498×440. -->
          <section id="screen-emoji" class="screen" role="region" aria-label="Эмодзи" aria-hidden="true">
            <div class="emoji-wrap">
              <div class="emoji-card" role="dialog" aria-modal="false" aria-label="Выбор эмодзи">
                <div class="emoji-head">
                  <search class="search-field" role="search">
                    <input id="emoji-search" class="search-input" type="search" placeholder="Найдите идеальный эмодзи" aria-label="Поиск эмодзи" autocomplete="off" />
                  </search>
                  <button class="icon-btn" type="button" data-action="emoji-close" aria-label="Закрыть">✕</button>
                </div>
                <div class="emoji-body">
                  <div id="emoji-tabs" class="emoji-tabs" role="tablist" aria-orientation="vertical" aria-label="Категории эмодзи"><!-- gen --></div>
                  <div id="emoji-grid" class="emoji-grid" role="listbox" aria-label="Эмодзи"><!-- gen --></div>
                </div>
                <div class="emoji-preview" aria-live="polite"><span class="emoji-preview-glyph" aria-hidden="true">🙂</span><span class="emoji-preview-name">Выберите эмодзи</span></div>
              </div>
            </div>
          </section>

          <!-- 8. Экран members: тулбар + виртуализованный список. -->
          <section id="screen-members" class="screen" role="region" aria-label="Участники" aria-hidden="true">
            <div class="members-toolbar">
              <search class="search-field" role="search">
                <input id="member-search" class="search-input" type="search" placeholder="Поиск участников" aria-label="Поиск участников" autocomplete="off" />
              </search>
              <span id="member-count" class="members-count" aria-live="polite">24 229</span>
            </div>
            <div id="member-list" class="list member-list" tabindex="0" role="list" aria-label="Участники" aria-busy="false" data-row-height="40"></div>
          </section>

          <!-- 9. Экран settings: оверлей с навигацией и контентом (контент генерит W39d). -->
          <section id="screen-settings" class="screen" role="region" aria-label="Настройки" aria-hidden="true">
            <div class="settings-shell" role="dialog" aria-modal="true" aria-label="Настройки пользователя">
              <header class="settings-head">
                <span class="avatar avatar--md" aria-hidden="true">SB</span>
                <span class="settings-user"><span class="settings-user-name">ShadowByte</span><span class="settings-user-hint">Редактировать профиль</span></span>
                <button class="icon-btn" type="button" data-action="settings-close" aria-label="Закрыть настройки">✕</button>
              </header>
              <div class="settings-body">
                <nav id="settings-nav" class="settings-nav" aria-label="Разделы настроек"><!-- gen --></nav>
                <div id="settings" class="settings" role="group" aria-label="Настройки"><!-- gen --></div>
              </div>
            </div>
          </section>
        </main>
      </div>
    </div>

    <script type="module" src="app.js"></script>
  </body>
</html>
```

Примечания к скелету:
- `.topbar` остаётся снаружи `main` (полноширинная строка окна), внутри — `.screen-title`
  и метрики `[F]`.
- `#chat-header` — sibling `.screen`, а не внутри `#screen-chat`: иначе при скрытом экране
  он бы исчезал вместе с регионом. Видимость переключается `body[data-active-screen="chat"]`.
- `#member-list` живёт только в `#screen-members`; панель чата использует отдельный
  `#member-panel-list` (превью, без PageStore) — см. §6.4.
- `#settings` — контейнер контента: `h3` внутри него ровно 3 `[F]`, поэтому заголовки
  навигации — **не** `h3` (кнопки в `#settings-nav`).
- `#emoji-search` и `#member-search` — сами `<input>` (не обёртки): тесты/фронт читают
  `.value`.
- `.visually-hidden` — утилита (см. `primitives.css`), не токен.

### 3.1 Правила видимости (CSS-контракт)

```css
html, body { height: 100%; margin: 0; overflow: hidden; }
.topbar { height: var(--layout-titlebar-h); }            /* 32 */
.shell { display: flex; height: calc(100% - var(--layout-titlebar-h)); }
.workspace { display: flex; flex: 1; min-width: 0; }
.screen { display: none; }
.screen.active { display: flex; flex-direction: column; min-height: 0; }
#screen-chat.screen.active { flex-direction: row; }
#chat-header { display: none; }
body[data-active-screen="chat"] #chat-header { display: flex; }
body[data-active-screen="settings"] #channel-tree { display: none; }  /* оверлей настроек */
#member-panel { display: none; }                                /* внутри #screen-chat */
body[data-active-screen="chat"] #member-panel { display: flex; }
```

Правило: `#chat-header` не входит в `#screen-chat`, поэтому видимость задаётся через
`body[data-active-screen]`; при скрытом экране `clientHeight` списков равен 0 (как в шиме) —
это условие теста «скрытый экран рисует только overscan» `[F]`.

## 4. Таблица элементов

### 4.1 Shell

| Элемент | id / класс | Роль / ARIA | Обязательные атрибуты |
|---|---|---|---|
| titlebar | `.topbar` | `role="banner"` | — |
| заголовок экрана | `.screen-title` | `role="heading" aria-level="2" aria-live="polite"` | текст `[F]` |
| метрики | `.metrics` | `aria-live="polite"` | `#m-fps/#m-p95/#m-rss/#m-threads` |
| бенч | `#bench.bench` | `button` | `type="button"` |
| shell-контейнер | `.shell` | — | flex-row, высота `calc(100% - 32px)` |
| рейл | `#guild-rail.rail` | `role="navigation" aria-label="Серверы и экраны"` | — |
| бренд | `.brand` | `aria-hidden="true"` | текст `DL` |
| гильдия | `.guild-item[data-guild]` | `button` | `aria-label`, `aria-current="true"` у выбранной |
| разделитель рейла | `.rail-sep` | `role="separator" aria-hidden="true"` | — |
| кнопка экрана | `.rail-btn[data-screen]` | `button` | `data-screen` ∈ 4, `aria-label`, `aria-current="page"` у активной, `title` |
| рабочая область | `.workspace` | — | flex-row |
| дерево | `#channel-tree.tree` | `role="navigation" aria-label="Каналы сервера"` | — |
| строка пользователя | `#user-bar.user-bar` | `contentinfo` (неявно `footer`) | — |
| контент | `main.content` | `main` | — |

### 4.2 Дерево каналов

| Элемент | Класс | Роль / ARIA | Примечание |
|---|---|---|---|
| шапка дерева | `.tree-header` | — | 48px `[A]` |
| имя сервера | `.tree-guild` | `h2` | 16/22 600 |
| меню сервера | `.icon-btn[data-action="guild-menu"]` | `aria-haspopup="menu" aria-expanded` | поповер §7.14 SPEC |
| скролл | `.tree-scroll` | — | `overflow-y:auto` |
| группа | `.channel-group` | `section` | — |
| заголовок группы | `.channel-group-title > .group-toggle` | `h3` + `button aria-expanded` | 12/16 uppercase |
| список | `.channel-list` | `role="list"` | `li` дети |
| канал | `.channel-item` | `button` | `aria-current="page"` у активного; `.is-unread` — непрочитанный; `data-channel-id` ставит W39d в рантайме (в статическом html нельзя, §9.2 п.4) |
| иконка | `.channel-icon` | `aria-hidden="true"` | `#`/🔊 |
| имя | `.channel-name` | — | ellipsis |

### 4.3 Чат

| Элемент | id / класс | Роль / ARIA | Примечание |
|---|---|---|---|
| шапка | `#chat-header.chat-header` | `header` | 49px, видна только на chat |
| заголовок | `.chat-title` (`h1`) + `.hash` + `.chat-title-text` | — | `#` декоративный `aria-hidden` |
| действия | `.chat-actions` | — | icon-btn ×4 + search (ветки · **колокол** · закреплённые · участники, VW-01/D-17) |
| колокол | `.icon-btn[data-action="inbox"]` | `button`, `aria-label="Уведомления" aria-disabled="true"` | Phase 3; подсказка `title` обязательна (VW-01) |
| поиск | `#chat-search.search-input` | `aria-label="Поиск сообщений"` | 32px pill |
| регион | `#screen-chat.screen.active` | `role="region" aria-label="Чат" aria-hidden="false"` | `[F]` |
| колонка чата | `.chat-main` | — | flex-column, min-width 0 |
| список | `#chat-list.list.chat-list` | `role="list" aria-label="Сообщения" aria-busy` | `tabindex="0"`, `data-row-height="44"` `[F]` |
| уведомление | `.chat-notice` | `role="note"` | 24px, `--text-muted` |
| композер | `#composer.composer` | `role="group" aria-label="Поле ввода сообщения"` | 56px |
| поле | `#composer-input.composer-input` | `textarea`, `aria-describedby="composer-hint"` | 1 строка |
| подсказка | `#composer-hint.visually-hidden` | — | текст для SR |
| панель | `#member-panel.member-panel` | `role="complementary" aria-label="Участники сервера"` | 240px, только chat |
| заголовок панели | `.member-panel-title` | `h2` | «Участники — N» |
| превью | `#member-panel-list.member-panel-list` | `role="list"` | без виртуализации |
| «все» | `.member-panel-all[data-action="all-members"]` | `button` | переход на members |

### 4.4 Эмодзи / участники / настройки

| Элемент | id / класс | Роль / ARIA | Примечание |
|---|---|---|---|
| карточка | `.emoji-card` | `role="dialog" aria-modal="false" aria-label="Выбор эмодзи"` | 498×440 |
| поиск | `#emoji-search` | `input[type=search]` | `[F]`: сам input |
| вкладки | `#emoji-tabs` | `role="tablist" aria-orientation="vertical"` | 10 `.tab` `[F]` |
| вкладка | `.tab[role="tab"]` | `aria-selected`, `aria-controls="emoji-grid"` | одна `.active` `[F]` |
| сетка | `#emoji-grid` | `role="listbox" aria-label="Эмодзи"` (D-07, frozen P1-2) | плоский список ячеек `role="option"` с непустым `aria-label`; без `role=row` |
| ячейка | `.emoji-cell[role="option"]` | `title`, `aria-label` | `title === textContent` `[F]` |
| предпросмотр | `.emoji-preview` | `aria-live="polite"` | 48px |
| тулбар участников | `.members-toolbar` | — | 48px |
| поиск | `#member-search` | `input[type=search]` | — |
| счётчик | `#member-count` | `aria-live="polite"` | — |
| список | `#member-list.list.member-list` | `role="list" aria-busy` | `data-row-height="40"` `[F]` |
| оверлей настроек | `.settings-shell` | `role="dialog" aria-modal="true"` | `[A]` размеры |
| шапка | `.settings-head` | — | 56px |
| навигация | `#settings-nav.settings-nav` | `nav` + `aria-label` | 232px; **без h3**; 18 `.settings-nav-item` + `.settings-nav-logout` (SET-01/D-17) |
| контент | `#settings.settings[data-active-pane]` | `role="group" aria-label="Настройки"` | 3 панели (`voice`/`appearance`/`notifications`) с одним `h3` каждая + 10 `h4.settings-block-title` в «Голосе» |

## 5. Состояния

### 5.1 `data-*`

| Атрибут | Где | Значения | Смысл |
|---|---|---|---|
| `data-active-screen` | `<body>` | `chat\|emoji\|members\|settings` | активный экран для CSS shell (на `.rail-btn` — `data-screen`, см. ниже) |
| `data-screen` | `.rail-btn` | `chat\|emoji\|members\|settings` | **только** на кнопках рейла (regex теста RW-38 собирает все `data-screen="…"` из html) |
| `data-density` | `<html>` (`:root`) | `cozy\|compact` | плотность сообщений (токен гаттера) |
| `data-state` | `.row`, `.member` | `row\|placeholder` (расширяемо: `empty`) | строка/скелетон |
| `data-group` | `.row` | `start\|continuation` | начало группы / продолжение |
| `data-index` | `.row`, `.member` | целое | сквозной индекс (отладка/тесты) |
| `data-row-height` | `#chat-list`, `#member-list` | `44`, `40` | шаг виртуализации |
| `data-action` | кнопки | см. §8.1 | имя действия |
| `data-channel-id`, `data-guild` | канал/гильдия | id | источник данных |
| `data-status` | `.status-dot` | `online\|idle\|dnd\|offline\|streaming` | статус |
| `aria-busy` | `#chat-list`, `#member-list` | `true\|false` | загрузка страницы |

### 5.2 Классы-состояния

| Класс | Где | Смысл |
|---|---|---|
| `.active` | `.screen`, `.rail-btn`, `.tab` | выбран/показан `[F]` |
| `.is-selected` | `.guild-item`, `.channel-item`, `.member` | выбранный объект (без смены экрана) |
| `.is-unread` | `.channel-item` | непрочитанный канал |
| `.is-muted` | `.channel-item` | приглушённый |
| `.is-loading` | контейнеры | скелетон |
| `.is-empty` | контейнеры | пустое состояние (внутри — `.empty-state`) |
| `.has-error` | контейнеры | ошибка (внутри — `.error-state`) |

Правило: класс — для CSS, ARIA — для AT. Дублировать состояние **обязательно**:
`.active` ↔ `aria-current="page"` (экраны) / `aria-selected="true"` (вкладки),
`.is-selected` ↔ `aria-current="true"` (гильдия/канал).

### 5.3 `aria-*`

| Атрибут | Где | Значение |
|---|---|---|
| `aria-hidden` | `.screen` | `"true"` у неактивных `[F]` |
| `aria-current` | `.rail-btn`, `.channel-item`, `.guild-item` | `page` / `true` / `false` |
| `aria-selected` | `.tab`, `.emoji-cell` | `true/false` |
| `aria-expanded` | toggle-кнопки, `group-toggle`, `[data-action=guild-menu]` | `true/false` |
| `aria-pressed` | mic/deafen | `true/false` |
| `aria-checked` | `input[role=switch]` | синхронно с `checked` |
| `aria-label` | иконки-кнопки, строки, регионы | текст на русском |
| `aria-live` | метрики, счётчики, `.screen-title` | `polite` (ошибки — `assertive`) |
| `aria-busy` | списки | `true` во время подгрузки |
| `aria-setsize` / `aria-posinset` | `.row`, `.member` | размер и позиция в полном списке (рекомендуется) |

## 6. Виртуализация и окно данных

### 6.1 Геометрия

```
#chat-list   (scroll container, tabindex=0, role=list, aria-busy)
└── div.sizer                 position:relative; height: top(totalRows)   ← ровно 1 ребёнок [F]
    ├── div.row[data-index=5412]  position:absolute; left:0; right:0; top:top(5412); height:height(5412)
    └── …
```

- Формулы (D-04): `sizerHeight = top(totalRows)`; `top(i) = Σ height(k), k < i`;
  `height(i) = 26/48 + 22·(lines − 1)` для чата и `42` для участников.
  Смещения считаются по периоду высот (`HeightIndex` в `ui/model.js`), а не
  массивом на 100 000 строк.
- Окно: `first = max(0, indexAt(scrollTop) − OVERSCAN)`,
  `last = min(totalRows, indexAt(scrollTop + clientHeight) + 1 + OVERSCAN)`,
  `OVERSCAN = 8`. При 800px видимого: ~24 строки данных + 17 overscan ≈ 41
  (лимит теста ≤ 60 `[F]`).
- Строки **переиспользуются** (пул по индексу окна); `replaceChildren()` только в
  пустом/ошибочном состоянии — иначе `sizer.children.length === rows.length` `[F]`.
- Скролл: только контейнер списка; `scrollTop` не «прыгает» при resize
  (сохранять anchor по индексу первой строки, `[A]`).
- Стартовое положение чата — конец списка (`scrollToIndex(totalRows − 1)` → `top(last)`), участников — 0 `[F]`.
- `#chat-list`/`#member-list` не имеют других детей: `.chat-notice`, тулбары и панели —
  снаружи. Пустое/ошибочное состояние — **оверлей в родителе** (`.chat-main`,
  `#screen-members`), не ребёнок списка: `#chat-list.children.length === 1` `[F]`.

### 6.2 PageStore (ADR-1, RW-37)

- `pageSize = 100`, `maxPages = 5` → `loadedRows ∈ {100,200,300,400,500}`, `≤ 500` `[F]`.
- `chat.totalRows = 100 000`, `members.totalRows = 24 229` `[F]`.
- `ensureRange(first,last)` вызывается до отрисовки окна; на RangeError — показать
  `.error-state` и `console.error` (тест ловит warning’и — не логировать в норме).
- `UI_SNAPSHOT.loadedRows` — из `store.stats().loadedRows` (не из DOM).

### 6.3 Где что рендерится

| Список | Контейнер | Рендер | Высота | Виртуализация |
|---|---|---|---|---|
| сообщения | `#chat-list > .sizer` | W39d `features/chat.js` | 26/48/+22 `[F]` (D-04) | да |
| участники (экран) | `#member-list > .sizer` | W39d `features/members.js` | 42 `[F]` (D-04) | да |
| участники (панель) | `#member-panel-list` | W39d, простой цикл | 42 `[L]` | нет (≤50) |
| эмодзи | `#emoji-grid` | W39d `features/emoji.js` | 44 | нет (≤120) `[F]` |
| настройки | `#settings` | W39d `features/settings.js` | 48 | нет (38) `[F]` |
| дерево | `.tree-scroll` | W39d `features/tree.js` | 28 | нет (272 канала `[L]`, скролл контейнера) |

### 6.4 Почему панель участников — отдельный список

`#member-list` обязан находиться внутри `#screen-members`: тест «скрытый экран рисует
только overscan» опирается на `clientHeight = 0` у скрытого `.screen`. Если перенести
`#member-list` в панель чата, инвариант ломается (список будет видим на chat).
Поэтому панель чата имеет собственный `#member-panel-list` без PageStore (первые ≤50
участников из загруженной страницы). Унификация — P3-03, отдельной карточкой.

## 7. Шаблоны строк (генерирует W39d)

### 7.1 Строка чата

```html
<div class="row" role="listitem" data-state="row" data-group="start"
     data-index="5412" aria-setsize="100000" aria-posinset="5413"
     aria-label="sasha, 20:50: #5412 привет …"
     style="position:absolute;left:0;right:0;top:182644px;height:48px">
  <span class="author">
    <span class="avatar avatar--lg" aria-hidden="true" style="--avatar-hue:212">S</span>
    <span class="author-name">sasha</span>
    <span class="badge badge--bot">BOT</span>
    <time class="timestamp" datetime="2026-10-23T20:50:00Z">20:50</time>
  </span>
  <span class="text"><span class="text-body">#5412 привет …</span></span>
</div>
```

- `data-group="continuation"` — `.author` присутствует в DOM, но скрыт CSS
  (`display:none`), `.text` центрируется; число детей остаётся 2 `[F]`.
- Placeholder: `<div class="row" data-state="placeholder" style="…height:height(index)px"></div>`
  (высота та же, что у будущей строки: 26/48/+22)
  — **без детей** `[F]`.
- `.text` всегда один; его `textContent` начинается с `#<index>` (оракул теста) `[F]`.
- Время/бейдж — внутри `.author` (не отдельными детьми строки) `[F]`.

### 7.2 Строка участника

```html
<div class="member" role="listitem" data-state="row" data-index="12"
     aria-setsize="24229" aria-posinset="13"
     aria-label="member_12_α, в сети, DOTA"
     style="position:absolute;left:0;right:0;top:504px;height:42px">
  <span class="avatar avatar--md" aria-hidden="true" style="--avatar-hue:154">M<span class="status-dot" data-status="online"></span></span>
  <span class="member-name"><span class="member-label">member_12_α</span><span class="badge badge--role">DOTA</span></span>
  <span class="member-role">играет в Dota 2</span>
</div>
```

- Ровно 3 ребёнка `[F]`; `children[1].textContent` начинается с `member_<i>_` `[F]`.
- Панель использует тот же шаблон с `height:42px` и `data-window="preview"`.

### 7.3 Вкладка и ячейка эмодзи

```html
<!-- 10 штук, по одной на категорию -->
<button class="tab active" role="tab" type="button" id="emoji-tab-0"
        aria-selected="true" aria-controls="emoji-grid" tabindex="0"
        title="Часто используемые" aria-label="Часто используемые">🕘</button>

<!-- Плоский список: строки-обёртки не нужны (D-07, #emoji-grid = listbox) -->
<button class="emoji-cell" role="option" type="button" tabindex="-1"
        title="😀" aria-label="улыбка">😀</button>
```

- `title` = глиф = `textContent` `[F]`; `aria-label` = человекочитаемое имя.
- Ровно один `.tab.active`/`aria-selected="true"` `[F]`; ячеек ≤ 120 `[F]`.

### 7.4 Настройки

```html
<h3>Голос и видео</h3>                        <!-- ровно 3 заголовка, тексты [F] -->
<label class="toggle">
  <span class="toggle-label">Шумоподавление <span class="toggle-hint">Krisp</span></span>
  <input class="toggle-input" type="checkbox" role="switch" aria-checked="true" checked />
  <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
</label>
```

- 38 `.toggle` и 38 `input[type=checkbox]`, все `checked` при старте `[F]`.
- `role="switch"` и `aria-checked` синхронизируются на `click` **и** `change`.
- Навигация `#settings-nav` — кнопки, не `h3` (иначе счётчик групп `[F]` ломается).

## 8. События и IPC

### 8.1 `data-action` (делегирование на `document`)

| `data-action` | Элемент | Действие |
|---|---|---|
| `guild-menu` | кнопка в `.tree-header` | открыть/закрыть поповер сервера |
| `toggle-tree` | шапка чата | свернуть/развернуть `#channel-tree` (`aria-expanded`) |
| `threads` / `pins` / `members` | шапка чата | панель/поповер (Phase 3) |
| `inbox` | колокол в шапке чата | приёмник уведомлений (Phase 3); кнопка `aria-disabled`, клик — no-op |
| `attach` | composer | файл-диалог (Phase 3), сейчас `aria-disabled` |
| `emoji` | composer | открыть `#screen-emoji` (как Ctrl+E) |
| `gift` | composer | заглушка-поповер (Phase 3) |
| `emoji-close` | карточка эмодзи | вернуться в чат |
| `all-members` | панель участников | перейти на `members` |
| `profile` | `#user-bar` | поповер профиля |
| `mute` / `deafen` | `#user-bar` | toggle `aria-pressed` (локально) |
| `settings` | `#user-bar` | открыть `settings` |
| `settings-close` | шапка настроек | закрыть (в chat) |

Клик по `.rail-btn` обрабатывается напрямую (`data-screen`), как в RW-38.

### 8.2 Клавиатура

`Alt+1…4` — экраны; `Ctrl+E` — эмодзи (toggle); `Ctrl+,` — настройки;
`Ctrl+F` — поиск с префиллом канала (поле «в»), `Ctrl+Shift+F` — без префилла; `/` — фокус
в `#chat-search`; `Ctrl+U` — панель участников (toggle); `Escape` — закрыть верхний слой,
иначе chat; `Shift+Esc` — сбросить плашку непрочитанного; `ArrowUp/Down`, `Home/End`,
`PageUp/PageDown` — списки/сетка/меню; `Enter/Space` — активация.
Обработчики не должны перехватывать ввод в `textarea`/`input` (кроме `Escape`).

### 8.3 IPC (не меняется)

`spike_config`, `process_metrics`, `report_ui_snapshot`, `report_bench`, `window_ready`.
Новые команды Phase 3 (W39e): `ready`, `messages_page`, `members_page`, `settings` —
вызываются из `features/*.js`, ошибки типизированы; при отсутствии `window.__TAURI__`
(шим) — локальные данные.

## 9. Совместимость с RW-38: блокер и решение

### 9.1 Проблема

Тест «шим-разметка совпадает с `ui/index.html` по id, классам и rail-кнопкам» собирает
**все** `id="…"` и `class="…"` из `index.html` и требует, чтобы каждый из них существовал
в DOM шима. Шим строит DOM вручную (`tests/support/gui-dom-harness.mjs:buildDom`), а новых
shell-узлов (требование ТЗ §4) в нём нет. Значит, любой новый id/класс в `index.html`
делает тест красным — **без** правки харнесса Phase 2 не может добавить shell.

Это не «ослабление теста»: тест защищает от дрейфа шима и разметки; аддитивное добавление
тех же узлов в `buildDom` сохраняет его смысл.

### 9.2 Требуемая правка (Plan A, рекомендуется)

`tests/support/gui-dom-harness.mjs`, функция `buildDom` — добавить узлы, зеркалящие §3.
Точный список (сгенерирован из скелета §3; при правке скелета — пересчитать):

**id (29):** `m-fps`, `m-p95`, `m-rss`, `m-threads`, `bench`, `guild-rail`, `channel-tree`,
`user-bar`, `chat-header`, `chat-search`, `screen-chat`, `chat-list`, `composer`,
`composer-input`, `composer-hint`, `member-panel`, `member-online`, `member-panel-list`,
`screen-emoji`, `emoji-search`, `emoji-tabs`, `emoji-grid`, `screen-members`,
`member-search`, `member-count`, `member-list`, `screen-settings`, `settings-nav`,
`settings`.

**Классы (76):** `active`, `avatar`, `avatar--md`, `avatar--sm`, `bench`, `brand`,
`channel-group`, `channel-group-title`, `channel-icon`, `channel-item`, `channel-list`,
`channel-name`, `chat-actions`, `chat-header`, `chat-list`, `chat-main`, `chat-notice`,
`chat-title`, `chat-title-text`, `composer`, `composer-attach`, `composer-input`, `content`,
`emoji-body`, `emoji-card`, `emoji-grid`, `emoji-head`, `emoji-preview`,
`emoji-preview-glyph`, `emoji-preview-name`, `emoji-tabs`, `emoji-wrap`, `group-toggle`,
`guild-item`, `hash`, `icon-btn`, `is-selected`, `is-unread`, `list`, `member-list`,
`member-panel`, `member-panel-all`, `member-panel-list`, `member-panel-title`,
`members-count`, `members-toolbar`, `metrics`, `rail`, `rail-btn`, `rail-sep`, `screen`,
`screen-title`, `search-field`, `search-input`, `settings`, `settings-body`, `settings-head`,
`settings-nav`, `settings-shell`, `settings-user`, `settings-user-hint`, `settings-user-name`,
`shell`, `topbar`, `tree`, `tree-guild`, `tree-header`, `tree-scroll`, `user-actions`,
`user-bar`, `user-meta`, `user-name`, `user-profile`, `user-status`, `visually-hidden`,
`workspace`.

Правила правки:
1. `buildDom` должен повторять §3 **один-в-один** по id/классам; вложенность — как в §3.
2. `boxHeight` сохранить: `#chat-list = 800`, `#emoji-grid = 600`, `#member-list = 800`,
   `#settings = 800`; новым спискам (`#member-panel-list`) `boxHeight` не нужен.
3. `#chat-list`/`#member-list` остаются пустыми (sizer создаёт `app.js`).
4. **Никаких `data-*id="…"` в статическом html**: regex теста `/id="([^"]+)"/` ловит
   `data-channel-id="…"` как id и требует такой элемент в шиме. Идентификаторы каналов
   W39d ставит в рантайме (runtime-разметка не сканируется).
5. Файл — вне матрицы W39b/W39c: правку выполняет владелец тестов по решению PM
   (или W39c в том же коммите с явным разрешением). Тест не переписывается,
   добавляются только узлы.
6. Порядок выката: правка харнесса + новый `index.html` — **одним** изменением,
   иначе тест 1 красный между коммитами.
7. По `TEST-POLICY.md` §2 это **не** ослабление теста: ни один `assert` не удаляется и не
   меняется, `buildDom` лишь догоняет новую разметку. Правка всё равно требует письменного
   решения PM (`DECISIONS.md`) и отдельного ревью тестового дифа; без него — Plan B (§9.3).

**Числа строк (D-04).** Замороженные `44`/`40` в §3 — значения атрибутов
`data-row-height` `[F]`, их не пересчитываем. Фактическая высота строк в рантайме:
чат — продолжение `26` + `22` за перенос (`--row-h-continuation`, `--row-line-extra`),
начало группы `48` (`--row-h-group-start`); участник — `42` (`--row-h-member`).
`buildDom` создаёт `#chat-list`/`#member-list` пустыми (§9.2 п. 3), поэтому высоты строк
в харнессе не хардкодятся: их задаёт `ui/model.js` через inline `top`/`height`.

**`#emoji-grid` (D-05).** Целевая структура — `grid` + строки-обёртки `row` с
`display: contents` (это закрывает структурную часть OBS-01, визуально ничего не меняя);
строки и ячейки генерирует W39d в рантайме, `buildDom` добавляет только контейнер.
ARIA-роль контейнера на этом шаге остаётся `listbox` по D-07/`[F]` (тест RW-38 проверяет
`role="listbox"` в `index.html`); смена роли на `grid` — отдельное решение PM, не в этой карточке.

### 9.3 Plan B (если правка тестов запрещена)

`index.html` остаётся с текущими id/классами, а shell-узлы создаёт `app.js` из
шаблонной строки при старте (до первого рендера). Тогда:
- тест 1 зелёный (в html нет новых id/классов); харнесс получает новые узлы уже в рантайме
  и тоже зелёный;
- минусы: разметка живёт в JS (дублирование с CSS-контрактом), SSR/просмотр HTML
  невозможен, ревью вёрстки сложнее, риск расхождения с §3;
- применять только по прямому указанию PM; в отчёте пометить как технический долг.

### 9.4 Ограничения DOM-шима (важно для W39d)

Шим поддерживает только селекторы: `#id`, `.class`, `tag`, `tag.class`,
`[attr="value"]`, потомок через пробел и группы через запятую. **Не поддерживаются**
`:not()`, `>`, `+`, `:scope`, `:first-child`, `[attr^=]`, `:has()`.

Правила для `app.js`:
1. Поиск узлов — `document.getElementById(...)` и простые селекторы из списка выше.
2. Новые shell-узлы искать через `getElementById`; если узел отсутствует (старый шим) —
   `if (!node) return;`, без `throw` (иначе тесты упадут на `console.error`).
3. CSS-состояния (`body[data-active-screen]`, `:focus-visible`, `@media`) в JS не парсить —
   только выставлять атрибуты (`dataset`, `setAttribute`).
4. `document.documentElement` доступен (`html`), `document.body` — тоже; `dataset`
   поддерживается на обоих.
5. `innerHTML`/`insertAdjacentHTML` в шиме нет: строки/ячейки/тумблеры создавать
   `createElement` + `append`/`replaceChildren` (иначе тесты упадут).

### 9.5 Ревизия D-04: переменная высота строк (W39d)

RE-оракул `waves/RW-37/RE/layout.json` (`row_heights` / `row_heights_detail`):

| Значение | Высота | Где |
|---|---|---|
| продолжение группы | **26** | `message_cozy` |
| начало группы | **48** | `message_cozy_group_start` |
| перенос строки | **+22** | `message_extra_line` |
| участник | **42** | `member` (панель и виртуализация) |

Модель высот живёт в `ui/model.js` (`ROW_HEIGHTS`, `messageHeight()`,
`HeightIndex`, `createChatHeights()`/`createMemberHeights()`); геометрия
периодична (period = lcm(4, 17, 23) = 1564), поэтому `top(i)`/`indexAt(offset)`
считаются без массива на 100 000 смещений.

Демо-данные спайка (правило высот, не UI): группа из 4 сообщений (индекс 0 —
начало), +1 строка у `i % 17 === 9`, ещё +1 у `i % 23 === 9`. Примеры:

| Индекс | Высота | Почему |
|---|---|---|
| 0 | 48 | начало группы |
| 1 | 26 | продолжение |
| 9 | 70 | продолжение + 2 переноса (26 + 44) |
| 26 | 48 | продолжение + 1 перенос (26 + 22) |
| 32 | 70 | начало группы + 1 перенос (48 + 22) |

Ожидания `tests/unit/gui-spike-dom.test.mjs` изменены **только по высотам**
(константы 44/40 → пары из таблицы); остальные assert не ослаблены. Мутация
«вернуть константу 44 в `messageHeight()`» валит 3 теста (sizer, высоты строк,
скролл) — лог `waves/RW-39/PM/w39d/mutation-const44.log`.

Остаток вне зоны W39d (read-only): `chat.css` задаёт `.text { height: 40px }` и
`.text-body { white-space: nowrap }` — для 26/+22 W39d ставит inline-высоту и
`white-space: normal` у многострочных строк; канонические правила — W39c2.

## 10. Чек-листы

### 10.1 W39c (markup/CSS)

- [ ] §3 скопирован в `ui/index.html`; id/классы/ARIA не переименованы.
- [ ] `tokens.css` скопирован в `ui/components/tokens.css` (md5 в отчёте), значения не
      переопределены.
- [ ] `#chat-list` и `#member-list` — пустые контейнеры с `data-row-height`.
- [ ] `#settings` не содержит навигации; `#emoji-tabs`/`#emoji-grid` пустые (генерация).
- [ ] Размеры shell по SPEC §5.1; `body[data-active-screen]` переключает видимость
      `#chat-header`/`#channel-tree`.
- [ ] Ни одного `@font-face`, `url()`, логотипа/названия Discord, эмодзи-спрайта.

### 10.2 W39d (frontend)

- [ ] `switchScreen` ставит `.active`, `aria-hidden`, `aria-current` и
      `document.body.dataset.activeScreen` (в т.ч. на старте — `chat`) `[F]`.
- [ ] Виртуализация §6: sizer, `top = top(index)` (D-04: 26/48/+22, участник 42),
      `indexAt(offset)`, `OVERSCAN=8`, переиспользование строк.
- [ ] PageStore: `loadedRows ≤ 500`, кратно 100 `[F]`.
- [ ] `UI_SNAPSHOT` — все поля §2.10 без изменений.
- [ ] Реализованы `data-action` §8.1, клавиатура §8.2, emoji search/tabs, settings switch
      (`aria-checked`), member search.
- [ ] Нет `console.error`/`console.warn` в штатных сценариях.

### 10.3 Приёмка (PM/QA)

- [ ] `node --test tests/unit/gui-spike-dom.test.mjs` — зелёный (после Plan A).
- [ ] Визуальная сверка по SPEC §14 на скриншоте `scripts/gui-window-probe.py`.
- [ ] AT-SPI/DOM: роли list/listitem/tab/gridcell/region присутствуют; фокус-visible
      виден; Tab-порядок по SPEC §10.
- [ ] В `ui/` нет ассетов Discord; шрифты только системные.
- [ ] `loadedRows ≤ 500` в `UI_SNAPSHOT` на живом окне.
