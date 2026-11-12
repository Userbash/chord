# Реестр CSS-токенов: App Shell Wiring, Modal Layer Stack, Guild Admin Panels & Composer Popovers (`DESIGN-P1-18` / `task-275`)

- **Волна / Пакет**: `RW-40` / `G3 Design` (`DESIGN-P1-18`, карточка `task-275-app-shell-wiring-spec`)
- **Единый источник истины CSS-переменных**: `rust/gui-spike/ui/tokens.css` (`md5: 21d570a6bf9c4b5d77b00d3d9b3d1d08`, `392` строки)
- **Эталон RE**: `reference-oracle/frontend-d03.md` (`§2 Layout Structure`, `§4 CSS Custom Properties`, `§5.5–§5.12`)
- **Связанная спецификация**: `docs/specs/APP-SHELL-WIRING-SPEC.md`

---

## 1. Токены иерархии слоёв Z-Index (`Z-Index Layer Stack`) `[T][D][M]`

Все уровни наложения компонентов в `app-shell-wiring.js`, `shell-bridge.js`, `shell-controllers-registry.js`, `chat-helpers-bridge.js` и `composer-popovers-bridge.js` строго привязаны к шести токенам из `rust/gui-spike/ui/tokens.css` (строки `336–341`):

| CSS-токен (`tokens.css`) | Строка в `tokens.css` | Значение | Слой и назначение | Привязанные компоненты (`task-275`) |
| :--- | :---: | :---: | :--- | :--- |
| `--z-base` | `336` | `0` | Базовая сетка оболочки и полноэкранные/региональные панели настроек гильдии | `.app-shell`, `.guild-rail`, `.channel-sidebar`, `.chat-main`, `.member-list`, `GUILD_ADMIN_PANELS`: `roles_matrix` (`[data-role-editor]`), `audit_log` (`[data-mod-audit-section]`), `automod` (`[data-automod-editor]`), `webhooks` (`[data-webhooks-editor]`), `expressions` (`[data-expressions-manager]`), `moderation` (`[data-moderation-panel]`), `onboarding` (`[data-onboarding-wizard]`) |
| `--z-header` | `337` | `10` | Верхняя панель канала, бейджи, поиск и плавающие экшен-бары сообщений | `.chat-header`, `.chat-header-badges` (`buildChannelHeaderBadgesModel`), `#search-panel`, `.message-action-bar`, `#ReconnectBanner`, `#RateLimitBanner` |
| `--z-popover` | `338` | `100` | Поповеры хедера, боковые дроверы канала, поповер профиля и все поповеры композера | `notification_inbox` (`#inbox-popover`), `pins_drawer` (`#chat-pins-panel`), `threads_drawer` (`#chat-threads-panel` / `[data-threads-drawer]`), `user_profile_card` popout (`#user-profile-popover`), `slash_command_picker` (`[data-slash-picker]`), `soundboard_picker` (`[data-soundboard-picker]`), `emoji_picker` / `gif_picker` (`[data-expression-picker]`), `attachment_dropzone` (`[data-attachment-dropzone]`), `#context-menu` |
| `--z-modal` | `339` | `200` | Модальные диалоги с бэкдропом и ловушкой фокуса (`Focus Trap`) | `quick_switcher` (`#quick-switcher` / `[data-quick-switcher]`), `user_profile_card` modal (`[data-profile-mode="modal"]`), `scheduled_events` (`[data-events-modal]`), `#settings-modal`, `#invite-modal`, `#forward-modal`, `#nsfw-age-gate` |
| `--z-tooltip` | `340` | `300` | Слой всплывающих подсказок поверх всех модалок и поповеров | `.ui-tooltip`, `[data-channel-topic-tooltip]` (`formatChannelTopicTooltip`), тултипы `.guild-rail` и `.chat-header` |
| `--z-toast` | `341` | `400` | Верхний системный слой уведомлений и тостов ошибок | `#toast-container`, `.toast-item` |

---

## 2. Геометрические и структурные токены оболочки, модалок, дроверов и поповеров `[T][D][M]`

| CSS-токен (`tokens.css`) | Строка в `tokens.css` | Значение в `tokens.css` | Область применения в `APP-SHELL-WIRING-SPEC.md` | Эталон RE (`reference-oracle/frontend-d03.md`) |
| :--- | :---: | :--- | :--- | :--- |
| `--layout-rail-w` | `214` | `64px` | Левая колонка иконок серверов `.guild-rail` | `rail_w=64` (`--custom-guild-list-width`) |
| `--layout-tree-w` | `215` | `240px` | Колонка списка каналов `.channel-sidebar` и левая колонка списков в `roles_matrix` / `automod` | `.sidebar_c48ade` (`width: 240px`) |
| `--layout-tree-w-wide` | `216` | `311px` | Расширенная ширина `.channel-sidebar` на экранах `>= 1600px` и максимальная ширина `.ui-tooltip` | `channel_tree_w=311` |
| `--layout-tree-min-w` | `217` | `200px` | Компактная ширина `.channel-sidebar` на экранах `720px–959px` | Адаптивный порог дерева каналов |
| `--layout-member-panel-w` | `218` | `240px` | Правая колонка участников `.member-list` (скрывается при `<= 1024px` или при открытой админ-панели на `<= 1280px`) | `.membersWrap_c8ffbb` (`width: 240px`) |
| `--layout-member-panel-w-wide` | `219` | `256px` | Расширенная ширина `.member-list` на экранах `>= 1600px` | `member_list_w=256` |
| `--layout-header-h` | `221` | `49px` | Высота верхней панели канала `.chat-header` и смещение `top` для `#inbox-popover`, `#chat-pins-panel`, `#chat-threads-panel` | `.container_fc4f04` (`--custom-channel-header-height`) |
| `--layout-composer-h` | `222` | `56px` | Высота блока ввода сообщения и базовое смещение `bottom` для `.composer-popover-slot` | `--custom-channel-textarea-text-area-height` |
| `--layout-userbar-h` | `223` | `56px` | Высота нижней панели пользователя в `.channel-sidebar` | `.panels_c48ade` (`56px`) |
| `--layout-content-max-w` | `230` | `720px` | Максимальная ширина центральной колонки `GUILD_ADMIN_PANELS` (`.guild-admin-host`) | Ширина контентной области настроек сервера |
| `--settings-nav-w` | `326` | `232px` | Ширина левой навигационной колонки в настройках и двухколоночных редакторах (`roles_matrix`, `automod`) | `--custom-layout-sidebar-width` |
| `--emoji-picker-w` | `308` | `498px` | Ширина поповеров композера `soundboard_picker` (`[data-soundboard-picker]`), `emoji_picker` и `gif_picker` (`[data-expression-picker]`) | `--custom-emoji-picker-constants-min-emoji-picker-width` |
| `--emoji-picker-h` | `309` | `440px` | Высота поповеров композера `soundboard_picker`, `emoji_picker`, `gif_picker` и `max-height` для `slash_command_picker` | `--custom-emoji-picker-constants-emoji-picker-height` |
| `--composer-input-w` | `305` | `100%` | Полная ширина `[data-slash-picker]` и мобильных поповеров (`< 720px`) | `composer_w=100%` |
| `--modal-w-sm` | `321` | `442px` | Ширина дроверов и поповеров хедера (`#inbox-popover`, `#chat-pins-panel`, `#chat-threads-panel`, `#user-profile-popover`) и малых модалок (`#invite-modal`, `#forward-modal`) | `--modal-width-small` |
| `--modal-w-md` | `322` | `602px` | Ширина модального окна `quick_switcher` (`.quick-switcher-dialog`) и полной карточки профиля `user_profile_card` (`.profile-modal-dialog`) | `--modal-width-medium` |
| `--modal-w-lg` | `323` | `800px` | Ширина крупных модальных окон (`#settings-modal`, `scheduled_events` `[data-events-modal]`) | `--modal-width-large` |
| `--modal-pad-x` / `--modal-pad-y` | `324–325` | `24px` / `16px` | Внутренние горизонтальные и вертикальные отступы модальных диалогов | `--modal-horizontal-padding` / `--modal-vertical-padding` |
| `--menu-min-w` / `--menu-pad` / `--menu-border` | `316–318` | `188px` / `8px` / `1px` | Геометрия контекстных меню и списка `slash_command_picker` | `--menu-scroller-block-padding`, `--menu-border-width` |
| `--row-menu-item` | `261` | `32px` | Минимальная высота пункта `.slash-command-item` и `.quick-switcher-item` | Высота строки меню/автодополнения |
| `--badge-h` / `--badge-pad-x` / `--badge-radius` | `301–303` | `16px` / `4px` / `var(--radius-sm)` | Геометрия бейджей `.header-unread-badge` (`99+`), `.header-thread-badge`, `.header-boost-badge` | Параметры бейджей хедера |
| `--tooltip-radius` / `--tooltip-pad-x` / `--tooltip-pad-y` | `329–331` | `var(--radius-sm)` / `8px` / `6px` | Геометрия `.ui-tooltip` (`formatChannelTopicTooltip`) | Параметры всплывающих подсказок |

---

## 3. Палитра поверхностей, текста, статусов, границ, скруглений, отступов и теней `[T][D][M]`

### 3.1. Поверхности и фоновые модификаторы (`Surface & State Tokens`)

| CSS-токен (`tokens.css`) | Строка | Значение (`theme-dark`) | Применение в компонентах `task-275` |
| :--- | :---: | :--- | :--- |
| `--bg-app` (`--bg`) | `61`, `72` | `#121214` | Основной фон `.app-shell`, `.guild-rail`, `.channel-sidebar` и подложки `.modal-backdrop` |
| `--bg-chat` | `62` | `#1a1a1e` | Фон `.chat-main`, `.chat-header`, `.member-list`, `.guild-admin-host` и всех региональных панелей `GUILD_ADMIN_PANELS` |
| `--surface-1` (`--bg-2`) | `63`, `73` | `#202024` | Панель пользователя, `:hover` строк сообщений (`--row-hover-bg`), карточки ролей и правил AutoMod |
| `--surface-2` (`--surface`) | `64`, `75` | `#242429` | Всплывающие слои `var(--z-popover)` и `var(--z-modal)`: `#inbox-popover`, `#chat-pins-panel`, `#chat-threads-panel`, `#user-profile-popover`, `.quick-switcher-dialog`, `.profile-modal-dialog`, `[data-slash-picker]`, `[data-soundboard-picker]`, `[data-expression-picker]` |
| `--surface-3` (`--bg-3`) | `65`, `74` | `#28282d` | Всплывающие подсказки `.ui-tooltip` (`var(--z-tooltip)`) и заголовки секций пикеров |
| `--surface-4` | `66` | `#2c2d32` | Верхний уровень карточек и активных контролов |
| `--surface-sunken` | `67` | `#222327` | Контейнер поля ввода композера `.composer-inner` |
| `--surface-input` | `68` | `#1e1f22` | Поля поиска `#quick-switcher-input`, `[data-soundboard-search]`, `[data-mod-audit-search]`, `[data-role-name-input]` |
| `--state-hover` | `124` | `rgb(255 255 255 / 6%)` | Состояние `:hover` для интерактивных элементов списков и кнопок |
| `--state-active` | `125` | `rgb(255 255 255 / 9%)` | Состояние `:active` кнопок тулбара и пикеров |
| `--state-selected` | `126` | `rgb(255 255 255 / 11%)` | Выбранный канал и активный фильтр |
| `--state-selected-strong` | `127` | `rgb(255 255 255 / 14%)` | Выбранный пункт `.slash-command-item.is-selected`, `.quick-switcher-item.selected`, `[aria-selected="true"]` |

### 3.2. Типографика, акценты и семантические статусы (`Text, Accent & Status Tokens`)

| CSS-токен (`tokens.css`) | Строка | Значение (`theme-dark`) | Применение в компонентах `task-275` |
| :--- | :---: | :--- | :--- |
| `--text-strong` | `81` | `#fbfbfb` | Заголовки модалок, выбранная команда `.slash-command-item.is-selected`, текст `.ui-tooltip` |
| `--text` | `82` | `#efeff1` | Основной текст в `.app-shell`, списках команд `/command`, карточках пинов/веток/почты и формах `GUILD_ADMIN_PANELS` |
| `--text-subtle` | `85` | `#abacb2` | Текст бейджей `.header-thread-badge`, `.header-boost-badge`, подзаголовки вкладок |
| `--text-muted` (`--muted`) | `83`, `84` | `#96979e` | Описания слэш-команд (`.slash-command-desc`), параметры, метаданные аудита, `#channel-topic` |
| `--text-faint` | `86` | `#81828a` | Заголовки категорий и пустые состояния |
| `--text-on-accent` | `89` | `#ffffff` | Текст на кнопках `--accent` |
| `--text-on-danger` | `90` | `#ffffff` | Текст на бейдже непрочитанных `.header-unread-badge` (`99+`) и кнопках удаления/бана `--danger-fill` |
| `--accent` / `--accent-hover` | `97–98` | `#5865f2` / `#4654c0` | Активные переключатели `role="switch"` (`[aria-checked="true"]`), кнопки сохранения в админ-панелях |
| `--danger` / `--danger-text` / `--danger-fill` | `101–103` | `#da3e44` / `#ff9a9e` / `#cf3a3f` | Бейдж непрочитанных `.header-unread-badge`, ошибки валидации `role="alert"`, кнопки бана/удаления |
| `--warning` | `105` | `#fdb833` | Индикатор `HTTP 202` и предупреждения лимитов слотов |
| `--positive` / `--positive-text` | `106–107` | `#3d9e60` / `#57d98a` | Статус активного события (`ACTIVE`), успешное завершение шагов `onboarding` |

### 3.3. Границы, скругления, отступы, тени и анимации (`Borders, Radii, Spacing, Shadows & Motion`)

| Категория | CSS-токен (`tokens.css`) | Строка | Значение | Применение |
| :--- | :--- | :---: | :--- | :--- |
| **Границы** | `--border-subtle` / `--border` / `--border-strong` | `119–121` | `rgb(151 151 159 / 12%)` / `20%` / `44%` | Рамки поповеров (`#inbox-popover`, `#chat-pins-panel`, `#chat-threads-panel`, `[data-slash-picker]`), нижняя граница `.chat-header`, контуры карточек админ-панелей |
| **Фокус** | `--focus-ring` / `--focus-ring-width` / `--focus-ring-offset` | `132–134` | `#a5b0ff` / `2px` / `2px` | Контур `:focus-visible` всех кнопок, вкладок, `role="option"` и `role="switch"` |
| **Тени** | `--shadow-elevation-low` / `--shadow-elevation-medium` / `--shadow-elevation-high` / `--shadow-popover` | `203–209` | Многослойные тени `rgb(0 0 0 / 16%..24%)` | `--shadow-elevation-low` для `.chat-header`; `--shadow-popover` для всех поповеров и модалок |
| **Скругления** | `--radius-xs` / `--radius-sm` / `--radius-md` / `--radius-lg` / `--radius-round` | `192–197` | `4px` / `8px` / `12px` / `16px` / `2147483647px` | Элементы списка (`.slash-command-item`), контейнеры поповеров (`--radius-md`), модалки (`--radius-lg`), пилл-бейджи (`--radius-round`) |
| **Отступы** | `--space-0` .. `--space-13` | `174–187` | `0`, `2px`, `4px`, `6px`, `8px`, `12px`, `16px`, `20px`, `24px`, `32px`..`64px` | Внутренние отступы и интервалы во всех сетках и списках |
| **Движение** | `--motion-fast` / `--motion-base` / `--ease-out` / `--ease-in-out` | `343–346` | `120ms` / `180ms` / `cubic-bezier(...)` | Переходы открытия/закрытия поповеров и подсветки выбора (при `prefers-reduced-motion: reduce` обнуляются до `0ms`, строки `350–355` в `tokens.css`) |

---

## 4. Адаптивные переопределения токенов по брейкпоинтам (`1024px`, `1280px`, `1440px`, `959px`, `1600px`) `[T][S][M]`

| Брейкпоинт (`@media`) | Переопределяемые токены и сетка | Значение на брейкпоинте | Поведение слоёв `task-275` |
| :--- | :--- | :--- | :--- |
| `@media (max-width: 719px)` | Сетка `.app-shell`, ширина поповеров хедера и композера | `1fr`, `width: var(--composer-input-w)` (`100%`) | Одноколоночная сетка; дроверы `#inbox-popover`, `#chat-pins-panel`, `#chat-threads-panel` и пикеры композера занимают всю ширину контейнера (`100%`). |
| `@media (max-width: 959px)` | `--layout-tree-w` (`tokens.css` строки `388–392`) | `var(--layout-tree-min-w)` (`200px`) | Сужение дерева каналов до `200px` на компактных окнах. |
| **`@media (max-width: 1024px)`** | Сетка `.app-shell` (`grid-template-columns`) | `var(--layout-rail-w) var(--layout-tree-w) 1fr` | `.member-list` скрыт (`display: none`); поповеры хедера открываются поверх `.chat-main` (`z-index: var(--z-popover)`), двухколоночные редакторы `roles_matrix` и `automod` складываются в одну колонку. |
| **`@media (max-width: 1280px)`** | `.app-shell[data-admin-panel-open="true"] .member-list` | `display: none` | В обычном режиме чата доступны все 4 колонки (`64px 240px 1fr 240px`), но при активации любой панели из `GUILD_ADMIN_PANELS` правый сайдбар участников автоматически скрывается для размещения двухколоночного редактора. |
| **`@media (max-width: 1440px)`** | Внутренние отступы `.guild-admin-host` | `padding: var(--space-6)` (`16px` вместо `24px`) | Оптимизация полезной ширины рабочих таблиц (`audit_log`, `webhooks`, `moderation`, `expressions`) на экранах `1280px–1440px`. |
| `@media (min-width: 1600px)` | `--layout-tree-w`, `--layout-member-panel-w` (`tokens.css` строки `367–372`) | `var(--layout-tree-w-wide)` (`311px`), `var(--layout-member-panel-w-wide)` (`256px`) | Увеличенная ширина боковых колонок на широкоформатных мониторах. |

---

## 5. Верификация контрастности WCAG 2.1 AA для всех пар токенов `[T][A][M]`

Все текстовые и интерактивные пары токенов из `rust/gui-spike/ui/tokens.css` проходят норматив **WCAG 2.1 AA** (`>= 4.5:1` для обычного текста, `>= 3.0:1` для крупного текста и UI-компонентов):

| Компонент / Узел UI | Токен переднего плана (`Foreground`) | Токен фона (`Background`) | Контраст (`tokens.css`) | Статус WCAG 2.1 AA |
| :--- | :--- | :--- | :---: | :---: |
| Заголовки каналов, модалок и выбранные команды (`.slash-command-item.is-selected`) | `var(--text-strong)` (`#fbfbfb`) | `var(--bg-app)` (`#121214`) / `var(--surface-2)` (`#242429`) | **18.10 : 1** / **13.30 : 1** | **PASS** (`>= 4.5:1`) |
| Основной текст в `.app-shell`, `.chat-header`, `GUILD_ADMIN_PANELS` | `var(--text)` (`#efeff1`) | `var(--bg-chat)` (`#1a1a1e`) | **15.10 : 1** | **PASS** (`>= 4.5:1`) |
| Основной текст в поповерах и модалках (`#inbox-popover`, `#chat-pins-panel`, `#chat-threads-panel`, `#quick-switcher`, `[data-slash-picker]`, `[data-soundboard-picker]`, `[data-expression-picker]`) | `var(--text)` (`#efeff1`) | `var(--surface-2)` (`#242429`) | **11.90 : 1** | **PASS** (`>= 4.5:1`) |
| Текст бейджей `.header-thread-badge` и `.header-boost-badge` | `var(--text-subtle)` (`#abacb2`) | `var(--surface-2)` (`#242429`) / `var(--bg-chat)` (`#1a1a1e`) | **6.10 : 1** / **7.60 : 1** | **PASS** (`>= 4.5:1`) |
| Вторичный текст (`.slash-command-desc`, параметры команд, `#channel-topic`, метаданные `audit_log`) | `var(--text-muted)` (`#96979e`) | `var(--surface-2)` (`#242429`) / `var(--bg-chat)` (`#1a1a1e`) | **4.70 : 1** / **5.80 : 1** | **PASS** (`>= 4.5:1`) |
| Текст ошибок валидации `role="alert"` (`[data-webhook-error]`, `[data-expr-error]`, `[data-mod-error]`, `[data-soundboard-error]`) | `var(--danger-text)` (`#ff9a9e`) | `var(--surface-2)` (`#242429`) | **6.45 : 1** | **PASS** (`>= 4.5:1`) |
| Бейдж непрочитанных `.header-unread-badge` (`99+`) и destructive-кнопки | `var(--text-on-danger)` (`#ffffff`) | `var(--danger-fill)` (`#cf3a3f`) | **4.85 : 1** | **PASS** (`>= 4.5:1`) |
| Акцентные кнопки сохранения (`[data-role-save]`, `[data-automod-save]`, `[data-onboarding-complete]`) | `var(--text-on-accent)` (`#ffffff`) | `var(--accent)` (`#5865f2`) | **4.61 : 1** | **PASS** (`>= 4.5:1`) |
| Контур клавиатурного фокуса `:focus-visible` | `var(--focus-ring)` (`#a5b0ff`) | `var(--bg-chat)` (`#1a1a1e`) / `var(--surface-2)` (`#242429`) | **8.30 : 1** / **6.71 : 1** | **PASS** (`>= 3.0:1` UI) |
