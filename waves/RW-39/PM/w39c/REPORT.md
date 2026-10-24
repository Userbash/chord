# Отчёт W39c (Markup/CSS) — шаг 1: токены + shell-раскладка

Дата: 2026-10-23. PM: DeepSeek. Зона: CSS.

Ограничение PM этого шага: `ui/index.html`, `ui/app.js`, `ui/model.js`, `src/**` — read-only
(их дописывает W38f). Вёрстка `index.html` не начиналась.

## 1. Сделано

| Файл | Что |
|---|---|
| `rust/gui-spike/ui/tokens.css` | новый, вендоринг `waves/RW-39/DESIGN/tokens.css` (1:1 по именам/значениям) |
| `rust/gui-spike/ui/style.css` | переписан: titlebar 32, shell/workspace/content, правила видимости экранов (§3.1) |
| `components/primitives.css` | сброс, типографика, `.visually-hidden`, `.list`/`.sizer`, placeholder/empty/error, `.icon-btn`, `.badge`, `.avatar`+`.status-dot`, `.search-field`/`.search-input`, скроллбары, focus-visible |
| `components/rail.css` | рейл 64, бренд `DL`, гильдии 40, разделитель, `.rail-btn` 40 + индикатор активного |
| `components/tree.css` | дерево 240, шапка 48, группы каналов, строка канала 28, user bar 56 |
| `components/chat.css` | список сообщений (виртуализация), шаблон строки 44, continuation/gutter, hover/selected |
| `components/header.css` | шапка чата 49, `.chat-title`/`.hash`, действия, поиск-пилюля 32 |
| `components/composer.css` | композер 56, поле 40, плейсхолдер |
| `components/emoji.css` | карточка 498×440, рейл категорий 48, строки-ряды 9×44+4, предпросмотр 48 |
| `components/members.css` | панель 240 (превью 42), тулбар 48, список 40, строка участника |
| `components/settings.css` | шапка 56, навигация 232/пункт 34, контент-колонка 720, строка 48, switch 40×24/18 |

`header.css` и `composer.css` подключены через `@import` в начале `chat.css`, потому что список
`<link>` в DOM-CONTRACT §3 заморожен, а PM просил отдельные файлы для header/composer.

## 2. Блок → токен → значение (источник — `ui/tokens.css`)

Проверяется машинно: `node waves/RW-39/PM/w39c/geometry-check.mjs` → 31/31 в допуске
(`geometry.log`).

| Блок | CSS-правило | Токен | Значение |
|---|---|---|---|
| titlebar | `.topbar` | `--layout-titlebar-h` | 32px |
| rail | `.rail` | `--layout-rail-w` | 64px (допуск 64–72) |
| элементы рейла | `.brand`, `.guild-item`, `.rail-btn` | `--layout-guild-item`, `--layout-rail-item` | 40px |
| морф рейла | `.brand`, `.rail-btn` | `--radius-selected` | 14px |
| дерево | `.tree` | `--layout-tree-w` | 240px (≥1600px → 311px) |
| шапка дерева | `.tree-header` | `--space-11` | 48px |
| строка канала | `.channel-item` | `--row-channel` | 28px |
| user bar | `.user-bar` | `--layout-userbar-h` | 56px |
| шапка чата | `.chat-header` | `--layout-header-h` | 49px |
| шаг строки чата | inline из JS (RW-38) | `--row-message-track` | 44px (frozen) |
| line-height сообщения | `.text` | `--line-height-message` | 22px |
| gutter продолжения | `.row[data-group="continuation"]` | `--message-gutter` | 64px |
| notice | `.chat-notice` | `--layout-group-title-h` | 24px |
| композер | `.composer` | `--layout-composer-h` | 56px |
| поле ввода | `.composer-input` | `--input-height` | 40px |
| панель участников | `.member-panel` | `--layout-member-panel-w` | 240px (≥1600px → 256px) |
| строка участника (экран) | inline из JS | `--row-member-track` | 40px |
| строка участника (панель) | inline из JS | `--row-member` | 42px |
| аватары | `.avatar--sm/md/lg` | `--avatar-sm/md/lg` | 20/32/40px |
| тулбар участников | `.members-toolbar` | `--space-11` | 48px |
| карточка эмодзи | `.emoji-card` | `--emoji-picker-w/h` | 498×440px |
| рейл категорий | `.emoji-tabs`, `.tab` | `--emoji-rail-w`, `--emoji-section-h` | 48px, 32px |
| сетка эмодзи | `.emoji-row` | `--emoji-cols`, `--emoji-cell`, `--emoji-gap` | 9 × 44px + 4px |
| предпросмотр | `.emoji-preview` | `--space-11` | 48px |
| шапка настроек | `.settings-head` | `--space-12` | 56px |
| навигация настроек | `.settings-nav`, `.settings-nav button` | `--settings-nav-w`, `--settings-nav-item-h` | 232px, 34px |
| строка тумблера | `.toggle` | `--space-11` | 48px |
| переключатель | `.toggle-track`, `.toggle-thumb` | `--switch-w/h`, `--switch-thumb` | 40×24px, 18px |
| колонка контента | `.settings`, `h3`, `.toggle` | `--layout-content-max-w` | 720px |
| поиск | `.search-input` | `--search-height`, `--search-radius` | 32px, pill |
| фокус | `:focus-visible` | `--focus-ring-width/-offset` | 2px / 2px |

Магических px нет: `rg "[0-9]+px|[0-9]+rem"` по `style.css` + `components/*.css` — 0 совпадений
во всех значениях (единственные «1px»-подобные — токены `--menu-border`, `--focus-ring-width`).

## 3. Скрытые экраны и роли

```css
.screen { display: none; }
.screen.active { display: flex; }
.screen[aria-hidden="true"] { display: none; }   /* побеждает при равной специфичности */
[hidden] { display: none !important; }           /* primitives.css */
```

`display:contents` не используется нигде, кроме упоминаний в комментариях (`rg` — 2 совпадения,
оба в комментариях). `role`-контейнеры (`.chat-main`, `.member-panel`, `.emoji-card`,
`.settings-shell`, `.emoji-row[role=row]`) остаются flex/grid-блоками, поэтому строки сетки
эмодзи — реальные `display:grid`, а не «растворённые» узлы.

## 4. Расхождения текущего `ui/index.html` с DOM-CONTRACT §2–§3

Объективно, скрипт `waves/RW-39/PM/w39c/dom-diff.mjs` (списки §9.2: 29 id, 76 классов):

- **нет 14 id:** `guild-rail`, `channel-tree`, `user-bar`, `chat-header`, `chat-search`,
  `composer`, `composer-input`, `composer-hint`, `member-panel`, `member-online`,
  `member-panel-list`, `member-search`, `member-count`, `settings-nav`;
- **нет 61 класса:** `.shell`, `.workspace`, `.content`, `.tree*`, `.user-*`, `.guild-item`,
  `.rail-sep`, `.chat-header`, `.chat-title`, `.hash`, `.chat-actions`, `.chat-main`,
  `.chat-notice`, `.composer*`, `.member-panel*`, `.members-toolbar`, `.members-count`,
  `.emoji-wrap`, `.emoji-card`, `.emoji-body`, `.emoji-tabs`, `.emoji-preview*`,
  `.settings-shell/head/body/user*`, `.search-field`, `.search-input`, `.icon-btn`, `.avatar*`,
  `.badge`-соседи и т.д.;
- **лишний 1 класс:** `.tabs` (в §3 это `.emoji-tabs`).

Структура и ARIA (ручная сверка §3):

1. Нет `.shell`/`.workspace`; `.rail` и `main` — прямые дети `body`; `.topbar` внутри `main`,
   а не полноширинной строкой над shell; у `.topbar` нет `role="banner"`.
2. `body` без `data-active-screen`, `html` без `data-density="cozy"` → CSS-переключение
   `#chat-header`/`#channel-tree`/`#member-panel` не сработает до правки W38f.
3. `.screen-title`: `aria-level="1"` вместо `2`, нет `aria-live="polite"`; у `.metrics` нет
   `aria-live`; `#bench` и `.rail-btn` без `type="button"`.
4. Нет дерева каналов (`#channel-tree`, `.tree-header`, `.tree-scroll`, `.channel-group`,
   `.channel-item`, `.group-toggle`, `.channel-icon`, `.channel-name`) и user bar (`.user-bar`,
   `.user-profile`, `.user-meta`, `.user-name`, `.user-status`, `.user-actions`).
5. Нет гильдий `.guild-item[data-guild]` ×3 и `.rail-sep`; `aria-label` рейла
   «Экраны приложения» вместо «Серверы и экраны»; нет `id="guild-rail"`.
6. Нет шапки чата (`#chat-header`, `.chat-title`, `.hash`, `.chat-title-text`, `.chat-actions`,
   `#chat-search`), `.chat-main`, `.chat-notice`, композера (`#composer`, `#composer-input`,
   `#composer-hint`) и панели (`#member-panel`, `.member-panel-title`, `#member-online`,
   `#member-panel-list`, `.member-panel-all`).
7. `#chat-list`: нет `aria-busy="false"` и `data-row-height="44"`; `#member-list` — нет
   `aria-busy` и `data-row-height="40"`; нет `.members-toolbar`, `#member-search`,
   `#member-count`.
8. Экран эмодзи: нет `.emoji-wrap`/`.emoji-card` (`role="dialog"`)/`.emoji-head`/`.emoji-body`/
   `.emoji-preview`; `#emoji-tabs` имеет класс `.tabs` и нет `aria-orientation="vertical"`;
   `#emoji-grid` — `role="listbox"` вместо `role="grid"` + `aria-rowcount`/`aria-colcount`;
   `#emoji-search` без класса `.search-input` и без `autocomplete="off"`, плейсхолдер другой.
9. Экран настроек: нет `.settings-shell` (`role="dialog"`)/`.settings-head`/`.settings-body`/
   `#settings-nav`.
10. `<head>`: нет `<meta name="color-scheme">`, нет ссылок на `tokens.css`,
    `components/primitives.css` и `components/*.css`; `<title>` другой.

Инварианты §2 при этом целы: 4 `.screen` в порядке chat/emoji/members/settings, 4 `.rail-btn`
в том же порядке, `data-screen` только на `.rail-btn`, тексты `.screen-title` дословно.
Ни один пункт не правился — это зона W38f/PM.

## 5. Проверки

```bash
# red-first: до создания файла
node -e "...tokens.css..."                     # ENOENT → red (waves/RW-39/PM/w39c/red-tokens.log)
# green
node -e "...url(|@font-face в tokens.css..."   # tokens ok        (green-tokens.log)
rg -n "url\(|@font-face|discord" style.css components   # пусто: ассетов/бренда нет
md5sum waves/RW-39/DESIGN/tokens.css ui/tokens.css      # см. §6
node --test tests/unit/gui-spike-dom.test.mjs  # 21 pass / 0 fail (остался зелёным)
node waves/RW-39/PM/w39c/geometry-check.mjs    # 31/31 в допуске (geometry.log)
node waves/RW-39/PM/w39c/dom-diff.mjs          # расхождения §4 (dom-diff.log)
```

Дополнительно: баланс `{}`/`()` по всем 10 CSS-файлам — 0 расхождений; все 197 `var(--...)`
разрешаются в объявленные токены (скрипт в отчёте, «all var() resolved»).

Не запускалось (по ограничению шага): `scripts/gui-window-probe.py`, AT-SPI, визуальная сверка
png, `cargo`, окно. `npx tsc -p rust/gui-spike/ui/tsconfig.json` не имеет смысла: JS в этом шаге
не менялся.

## 6. md5

| Файл | md5 |
|---|---|
| `waves/RW-39/DESIGN/tokens.css` (источник) | `579a878aef43f8fa50a924eb4277acd9` |
| `rust/gui-spike/ui/tokens.css` (вендоринг) | `70915fff2e0a97cd4e0392a2241f4042` |
| `rust/gui-spike/ui/style.css` | `0f53490f9f48becd03a57fdd988a1546` |
| `components/primitives.css` | `943b3aa129f4cd952718b4d4191786d3` |
| `components/rail.css` | `92b4f40b0bc962e15e4275bd42eed749` |
| `components/tree.css` | `3f084ac11bca8e612dadf8a1d08e619d` |
| `components/chat.css` | `11a01cc14e4bda6823b9c742da9f5966` |
| `components/header.css` | `987d7b6b5643b558c4494a10639b8f67` |
| `components/composer.css` | `4aaefb2c9fe632367ee105d8cf878106` |
| `components/emoji.css` | `04ea9b04705c6e16204f4afda490f4a4` |
| `components/members.css` | `30074605780e75cc66404b7e38b90523` |
| `components/settings.css` | `3a767e0933bdee436d80beed45e48b3d` |

md5 вендоринга отличается от источника ровно на шапку файла и одну строку комментария:
в DESIGN-шапке было имя CSS-функции внешних ссылок, из-за которого текстовый гейт «нет ассетов»
срабатывал на самом комментарии. Заменено словами «внешних ссылок» — **только комментарий**.
Доказательство равенства значений: списки пар property/value, извлечённые grep-ом, совпадают
побайтово (`diff` пуст), значения не переопределены.

## 7. Осталось / риски (честно)

1. **`index.html` не начат** — жду сигнала PM, что W38f закончил. Вместе с ним нужны `<link>`:
   `tokens.css`, `components/primitives.css`, `style.css`, `components/rail.css`, `tree.css`,
   `chat.css`, `emoji.css`, `members.css`, `settings.css`.
2. **Отклонение от контракта §3/§10.1:** по указанию PM токены лежат в `ui/tokens.css`, а не
   в `components/tokens.css` → в `<link>` будет `href="tokens.css"`. Это правка контракта,
   а не самовольство вёрстки; нужно зафиксировать в §3 (строка `<link ... components/tokens.css>`)
   и в §10.1.
3. **Пока `index.html` старый, окно будет без стилей:** новый `style.css` рассчитан на
   `.shell`/`.workspace`/`.content` и `var(--...)` из ещё не подключённого `tokens.css`.
   Правка харнесса и новый `index.html` по §9.2 п.6 должны выйти одним изменением; до этого
   визуальную сверку делать бессмысленно.
4. **Строки «22–26/48»:** фактический шаг строки чата — 44px (frozen RW-38, §2.4/§6.1),
   `--row-message-cozy: 26px` и `--row-message-start: 48px` в tokens.css помечены как
   справочные RE. В CSS взяты line-height 22px и gutter 64px; менять шаг на 26/48 нельзя без
   правки DOM-теста. Если PM хочет RE-плотность — отдельная карточка, иначе DOM-тест красный.
5. Визуальная сверка (`gui-window-probe.py`, `/tmp/opencode/gui-window.png` vs
   `/home/sanya/re-deep/ui/00-app.png`) и AT-SPI не выполнялись — запрет на запуск окна
   на этом шаге. После замены `index.html` — прогон probe и сверка по SPEC §14.
6. Коммитов/пушей не делал, сеть не использовал, чужие файлы (`index.html`, `app.js`,
   `model.js`, `src/**`, тесты) не менял. `rust/gui-spike/ui/` в git пока untracked — это
   состояние до координатора.
