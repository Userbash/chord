# D-03 — UI Component Interaction Traces & Empirical Component Contracts

- **Environment**: Discord Desktop (Chrome 148 / Electron 42, build 1.0.162), CDP `http://127.0.0.1:9223`.
- **Methodology**: Native CDP automation via HTTP `/json/list` (`Connection: close` → WebSocket; `Runtime/Page/Input`), `getBoundingClientRect` interaction, JPEG/PNG screenshots with view clipping, and static webpack module extraction (`decoded-index.json`).
- **Boundaries**: Read-only inspection; no messages transmitted, voice connections avoided, user settings left unchanged; `logs/`, `functions/`, and `evidence/` directories untouched.
- **Artifact Provenance**:
  - `docs/re/ui/` & `/home/sanya/re-deep/ui/` (`00-app.png`, `01-msg-menu.*`, `03-quickswitcher.*`, `05-dm-sidebar.html`, `06-profile-popout.html`, `tokens.json`).
  - `docs/re/ui2/` & `/home/sanya/re-deep/ui2/` (`00-start.fingerprint.json`, `D1-guild-menu-items.json`).
  - `docs/re/ui3/` & `/home/sanya/re-deep/ui3/` (`A-dm-sidebar.html`, `A-dm-links.json`, `B-panels-candidates.json`, `B-popout-candidates.json`).
  - `docs/re/ui6/` & `/home/sanya/re-deep/ui6/` (`01-guild-channels.json`, `02-channel.png`, `05-popout-candidates.json`, `ui-trace6.mjs`).
  - `docs/re/ui8/` & `/home/sanya/re-deep/ui8/` (`20-search-results.*`, `20b-search-filters.*`, `20c-search-sort.*`, `21-emoji-picker.*`, `21b-emoji-full.*`, `21d-emoji-categories.*`, `22-members.*`, `23-settings-categories.json`, `23-settings-voice.*`; drivers `ui-search4..9.mjs`, `ui-emoji5..9.mjs`, `ui-settings7..13.mjs`, `ui-members6.mjs`).
- **CSS Variable Definitions**: Extracted from live client `ui/tokens.json` (4,795 dark-theme variables).

---

## 0. Channel Sidebar & Chat Scroll Virtualization

### 0.1 Channel Sidebar (Guild Channel Tree, Private DM List & User Status Panel)
- **Guild Rail (`wrapper_ef3116 guilds__5e434`)**:
  - Root element: `<nav class="wrapper_ef3116 guilds__5e434 theme-dark theme-darker images-dark" aria-label="Боковая панель серверов">`.
  - Hierarchy: `<div class="tree_ef3116" role="tree">` → `<div class="stack_dbd263" role="group" aria-label="Личные сообщения">` / `<div class="stack_dbd263" role="group" aria-label="Серверы">` → `<div class="wrapper__6e9f8 selected__6e9f8" role="treeitem">` (plus action items `circleIconButton__5bc7e` with `aria-label="Добавить сервер"` and `discoveryIcon_ef3116` `aria-label="Путешествие"`).
- **Guild Channel Tree (`01-guild-channels.json`)**:
  - Channel anchor nodes: `<a class="link__2ea32" href="/channels/{guild_id}/{channel_id}" aria-label="…">`.
  - Empirical ARIA labels:
    - Read text channel: `aria-label="💬welcome (текстовый канал)"`
    - Unread text channel: `aria-label="не прочитано, 🔤болталка (текстовый канал)"`
    - Forum channel: `aria-label="💾dpakc_help🖥💻 (канал форума)"`
  - Sorting & Category Collapse Contract:
    - Categories (`GUILD_CATEGORY = 4`) sort by `(position, id)`.
    - Within each category (and in the top uncategorized bucket), non-voice channels (`GUILD_TEXT = 0`, `GUILD_ANNOUNCEMENT = 5`, `GUILD_FORUM = 15`) sort before voice-like channels (`GUILD_VOICE = 2`, `GUILD_STAGE_VOICE = 13`), then by `position`, then by Snowflake `id`.
    - When a category is collapsed (`aria-expanded="false"`), read channels are hidden from the visible tree while channels with `unread == true` or `mention_count > 0` remain visible (`numberBadge__2ea32`).
- **Private Channels / DM Sidebar (`A-dm-sidebar.html`, `A-dm-links.json`)**:
  - Root container: `<nav class="privateChannels_e6b769" aria-label="Приватные каналы">`.
  - Search trigger bar: `<div class="searchBar_e6b769"><button data-mana-component="button" role="button" class="button_a22cb0 sm_a22cb0 secondary_a22cb0 hasText_a22cb0 fullWidth_a22cb0" type="button">Найти или начать беседу</button></div>` (shortcut `Ctrl+K` / `Cmd+K` → QuickSwitcher module `675704`).
  - Virtualized scroller: `<div class="scroller__99e7c scrollbarGutterStable_d125d2 thin_d125d2 scrollerBase_d125d2 fade_d125d2" tabindex="0" data-list-id="private-channels-uid_11" aria-orientation="vertical" data-jump-section="global">`.
  - List container: `<ul role="list" aria-label="Личные сообщения" class="content_d125d2" style="height: 905px;">`.
  - Top navigation rows (`<li class="channel__972a0 container_e45859" role="listitem" aria-setsize="22" aria-posinset="1..4">`):
    - `/channels/@me` (`Друзья`, `data-list-item-id="private-channels-uid_11___friends"`)
    - `/store` (`Nitro`, `data-list-item-id="private-channels-uid_11___nitro"`)
    - `/shop` (`Магазин`, `badge_c2b88c expressive_c2b88c` `"новое"`)
    - `/quest-home` (`Задания`)
  - Section header: `<h2 class="privateChannelsHeaderContainer__99e7c container__13cf1"><span class="headerText__99e7c">Личные сообщения</span><div class="headerActions__99e7c">…</div></h2>` with action buttons `moreButton__99e7c` (`aria-label="Ещё"`, `aria-haspopup="menu"`) and `privateChannelRecipientsInviteButtonIconContainer__99e7c` (`aria-label="Создать сообщение"`).
  - DM item rows (17 captured in `A-dm-links.json`, `aria-posinset="6..22"`):
    `<li class="channel__972a0 dm__972a0 container_e45859" role="listitem">` → `<div class="interactive_f88cfd interactive__972a0">` → `<a class="link__972a0 channelNameFade__972a0" href="/channels/@me/{channel_id}" tabindex="-1">` containing `layout__20a53` (`avatar__20a53` 32×32 `wrapper__44b0c` with `svg-mask-avatar-status-round-32` + `content__20a53` `name__20a53 text-md/medium__20a53` and optional clan tag `clanTag__972a0` `chipletContainerInner__10651`) + close button `<div class="closeButton__972a0" aria-label="Закрыть ЛС" role="button" tabindex="0">`.
- **Sidebar Resize Handle & User Status Panel (`B-panels-candidates.json`, `00-start.fingerprint.json`)**:
  - Resize handle: `<div class="sidebarResizeHandle__5e434" role="separator" aria-label="Resize Sidebar">`.
  - Bottom user panel: `<section class="panels__5e434" aria-label="Статус и настройки пользователя">` containing:
    - `<div class="accountPopoutButtonWrapper__37e49">` → `<div class="accountPopoutButton__37e49" role="button" aria-label="Управление профилем и статусом">` (opens User Popout) + `panelSubtextContainer__37e49`.
    - Audio & settings controls: `<button class="button__67645 audioButtonWithMenu__5e764" role="switch" aria-label="Заглушить">`, `<button class="button__67645 buttonChevron__5e764" aria-label="Настройки ввода">`, `<button class="button__67645 audioButtonWithMenu__5e764" role="switch" aria-label="Откл. звук">`, `<button class="button__67645 buttonChevron__5e764" aria-label="Настройки вывода">`, `<button class="button__67645" aria-label="Настройки пользователя">`.

### 0.2 Chat Scroll Virtualization
- **Scroller Container (`module 6095`)**:
  - Root scroller: `scrollerBase_d125d2 auto_d125d2 scrollbarGutterStable_d125d2 customTheme_d125d2` (`disableScrollAnchor` toggled during dynamic history prepends).
  - Virtualized list items container: `scroller_affa7e` + `listItems_affa7e` / `content_d125d2`.
- **Message Row DOM Hierarchy (`20-search-results.json`, `ui/01-msg-menu.html`)**:
  - Row wrapper: `<div class="wrapper_c19a55 cozy_c19a55 zalgo_c19a55">` → `<div class="contents_c19a55">`.
  - Cozy header (`header_c19a55`): avatar `avatar_c19a55 clickable_c19a55` (`wrapper__44b0c avatarStack__44b0c`), author `headerText_c19a55` → `username_c19a55 usernameColorOnName_c19a55`, optional bot badge `botTagRegular__82f07 botTag__82f07`, optional clan chiplet `clanTagChiplet_c19a55 chipletContainerInner__10651`, timestamp `timestamp_c19a55 timestampInline_c19a55`.
  - Message body: `<div class="markup__75297 messageContent_c19a55">` + accessories (`embedCard__44c9a`, `attachmentLink`, reaction pills, hover action toolbar `buttonsContainer__80bf8`).
- **Virtualization & Dynamic Sizing Contract**:
  - Default estimated row height: `48px` (cozy message row), dynamically updated via `measureElement(index, heightPx)` when multi-line markdown, embeds, or media attachments expand a row (e.g., `48px → 120px`).
  - Bounded DOM Pool: Even across `100,000` message streams (`4,800,000px` sizer height), the rendered DOM node pool is strictly capped at `<= 60` visible + overscan nodes (`CHAT_VIRTUALIZER_MAX_DOM_POOL = 60`).
  - Sticky Date Divider & Jump-to-Bottom Unread Pill:
    - `.chat-sticky-date-divider` pins the formatted date header of the topmost visible message (`firstVisibleIndex()`).
    - `.chat-jump-bottom-pill` remains `[hidden]` while anchored at the bottom (`scroll_top + row_height >= max_scroll_top`) and becomes visible when scrolled up, displaying accumulated unread counts and jumping back to `max_scroll_top` on click or `Escape`.

---

## 1. Search

### 1.1 Results (Guild Search)
- Text input method: `Ctrl+A` in search bar (clears scope token) followed by keystroke dispatch. Target container: contenteditable `editor__25bec`, results panel `searchResultsWrap_a98f3b`, header title `totalResults` (`searchHeader_ae7890`, `totalResults_ae7890`, `searchModeAndFiltersContainer_ae7890`).
- Empirical results: `ProBot` query yields 3,302 results; `Hello` (`Привет`) query yields 49,887 results.
- Structure: `scroller_a98f3b scrollbarGutterStable_d125d2 thin_d125d2 scrollerBase_d125d2` → `searchResultGroup_a7e67f` (aria "channel, category", `channelNameContainer_a7e67f`) → `container__80bf8` / `searchResult__80bf8` / `message__80bf8`; message row `wrapper_c19a55 cozy_c19a55 zalgo_c19a55`, `id="search-result-<id>"`, `<svg aria-label="Text">`.
- DOM inspection records 6 visible items (`itemCount=6`), classes: `searchResultGroup_a7e67f`, `container__80bf8`, `searchResult__80bf8`, `message__80bf8`, `wrapper_c19a55`.
- Artifacts: `20-search-results.{json,html,jpg}`, `20a-searchbar.json`, `20a-search-open.jpg`, `20a-search-typed.jpg`, `20e-search-typed-rules.jpg`, `20h-winner.json` (`ctrlA+typeChars` against `/api/v9/guilds/<gid>/messages/search`).

### 1.2 Filters
- Sections `filterSection_e4772e`: **From** (`От`), **In** (`В`), **Has** (`Содержит`), **Mentions** (`Упоминания`), **Date** (`Дата`), **Author Type** (`Тип автора`), **Pinned** (`Закреплено`) (7 sections).
- Inputs: `input__75098 comboBoxInput__0edde` with placeholders.
- Buttons: "Clear Filters" (`Очистить фильтры`), "Cancel" (`Отмена`), "Apply Filters" (`Применить фильтры`).
- "Has" options (9): image (`изображение`), video (`видео`), link (`ссылку`), file (`файл`), attachment/embed (`вложение`), audio/sound (`аудио`), poll (`опрос`), sticker (`стикер`), forward/snapshot (`переслать`).

### 1.3 Sorting
- Menu `menu_c1e9c4 flexible_c1e9c4`: "Newest" (`По новизне`, `aria-checked=true`), "Oldest" (`Самое старое`), "Most Relevant" (`Самое актуальное`) (3 options).

### 1.4 Network Characteristics
- With channel scope chip: `GET /api/v9/guilds/{id}/messages/search?channel_id={cid}&content=ProBot...` returns `{"messages":[],"total_results":0}`.
- Unscoped search: `...messages/search?content=ProBot...` returns `total_results=3302`.

---

## 2. Emoji

### 2.1 Full Picker (Shortcut `Ctrl+E`)
- Container: `positionLayer__08434 expressionPickerPositionLayer__74017 theme-dark images-dark layer__59d0d`.
- Tabs `[role=tab]`: "GIFs" (`Гифки`), "Stickers" (`Стикеры`), "Emoji" (`Эмодзи`, `aria-selected=true`).
- Search input `input__75098` with placeholder "Search emoji" (`Найдите идеальный эмодзи`); "Add Emoji" (`Добавить эмодзи`) button; skin tone button (`aria-label="Выбрать цвет кожи эмодзи (Пусто)"`).
- Grid: 50 visible `emojiItem_fc7141`, 17 `lockedEmoji_d982c2` (Nitro gated) in guild channel (`42` visible in DM full picker `21b-emoji-full.json`; `:fire` search yields `10` items in guild channel and `11` in DM).
- Category rail (`listItems_affa7e`, `navList__08434`, 10 categories): `top guild emoji`, `recent`, `ITSKIDON PC BOOST` (`guild`, `categoryItemGuildCategory_b9ee0c`), `people`, `nature`, `food`, `activity`, `travel`, `objects`, `symbols` (`categoryItem_b9ee0c` / `categoryPill_b9ee0c`).

### 2.2 Reaction Picker (Context Menu)
- Context menu option `message-add-reaction` triggers compact picker: `emojiPicker_c0e32c` + `bodyWrapper_c0e32c`, `emojiPickerListWrapper_c0e32c`, `listWrapper_c656ac`, `categorySection_c656ac`, 5 rows of `emojiListRow_fc7141`, 60 `emojiItem_fc7141`, 43 `lockedEmoji_d982c2`. Search input and tabs omitted.

---

## 3. User Popouts & Member Roster (Right Sidebar)

### 3.1 User Popout (`06-profile-popout.html`, `B-popout-candidates.json`)
- **Dialog Container**: `<div aria-labelledby="_r_fl_" class="popoutContainer_ce8328 accountPopout__37e49" data-layer="base" role="dialog" tabindex="-1" aria-modal="true">`.
- **Accessibility Live Announcer & Heading**:
  - Visually hidden live region `hiddenVisually_b18fe2` with `data-live-announcer="true"` (`role="log" aria-live="assertive"` and `role="log" aria-live="polite"`).
  - Visually hidden dialog heading `<h1 id="_r_fl_">Управление профилем и статусом</h1>`.
- **Theme & Gradient Wrapper**:
  - `<div class="outer_c0bea0 theme-dark theme-darker images-dark user-profile-popout themeContainer_ce8328">` defining custom properties `--profile-gradient-primary-color: var(--background-surface-high)`, `--profile-gradient-secondary-color: var(--background-surface-high)`, `--profile-gradient-overlay-color: rgba(0, 0, 0, 0)`, `--profile-gradient-button-color: var(--background-mod-subtle)`, `--profile-gradient-modal-background-color: var(--background-base-lower)`.
- **Header, Banner & 80×80 Status Avatar (`header__5be3e`)**:
  - Banner: `<div class="banner_f7e69e" style="height: 105px; width: 100%;"><div class="fill_f7e69e banner__68edb" style="--custom-cutout-radius: 46px; --custom-cutout-x: 56px; --custom-cutout-y: calc(100% - 4px);"></div></div>`.
  - Avatar: `<div class="avatar__75742 clickable__75742">` containing `<div class="focusTarget__54e4b" aria-label="Полный профиль" role="button" tabindex="0">` and `<div class="wrapper__44b0c" role="img" aria-label="pharos12510, Невидимый" style="width: 80px; height: 80px;">` with `<svg width="92" height="92" viewBox="0 0 92 92" class="mask__44b0c svg__44b0c">` (`mask="url(#svg-mask-avatar-status-round-80)"` + status rect `16×16` at `(60, 60)` `mask="url(#svg-mask-status-offline)"`).
  - Custom status bubble: `container_ab8609 editable_ab8609` → `outer_ab8609[role="button"][tabindex="0"][aria-label="Добавить пользовательский статус"]`.
- **Body Scroller, Identity & Action Menus (`body_ce8328`)**:
  - Scroller: `<div class="body_ce8328 scrollbarGutterStable_d125d2 thin_d125d2 scrollerBase_d125d2">`.
  - Identity (`userInfo_ce8328`):
    - `displayNameRow__26b1f`: display name `displayName__26b1f heading-lg/bold_cf4812` (`ShadowByte`) + copy button `copyButton__252af[role="button"][aria-label="Копировать имя пользователя"]`.
    - `usernameAndPronounsRow__26b1f`: tag `userTagUsername__26b1f` (`pharos12510`) + badge group `<div class="container__8061a" aria-label="Значки пользователя" role="group">` (`anchor__14d82` links with `badge__8061a` icons and `richTooltipContent__9038b` descriptions).
  - Menu overlays (`menus_ce8328` → `overlay_c0bea0 menuOverlay_ce8328` → `<ul>` → `<li class="menuItem_ce8328">` → `menuItemContent_ce8328[role="button"][tabindex="0"]`):
    - Group 1: `Редактировать профиль`, Status selector (`Ваш статус: Невидимый`, `aria-expanded="false"`, `submenuIconWrapper_ce8328`), App/Orb status item.
    - Group 2: `Переключение между учётными записями` (with `submenuIconWrapper_ce8328[aria-label="Другие настройки"]`), `Копировать ID пользователя`.

### 3.2 Member Roster (`22-members.json`)
- Container: `aside[class*=membersWrap]` → `membersWrap_c8ffbb hiddenMembers_c8ffbb` (`rect: 256×1025` at `x0=1792, y0=82`).
- Inner scroller: `members_c8ffbb scrollbarGutterStable_d125d2 thin_d125d2 scrollerBase_d125d2 fade_d125d2` → `content_d125d2`.
- Groups: "Server Booster — 2", "Online — 735" (`В сети — 735`, `membersGroup_c8ffbb container__13cf1` → `membersGroupHeader_c8ffbb` / `membersGroupName_c8ffbb`).
- Rows: 35 visible `member__5d473 member_c8ffbb container__91a9d clickable__91a9d` → `memberInner__5d473 layout__91a9d`; contains `avatar__91a9d` (`wrapper__44b0c avatarStack__44b0c avatar__44b0c`), name `name__91a9d text-md/medium__91a9d` (`username__5d473`, `desaturateUserColors__41f68`), clan tag `clanTag__5d473` (`chipletContainerInner__10651`), and activity status `subText__91a9d` (`customStatusContent_c9d15c`).

---

## 4. Settings

### 4.1 Navigation Hierarchy
- Trigger: Button with `aria-label="User Settings"` (`Настройки пользователя`) in user panel (`panels__5e434`).
- Root: `container_abd9a8` (1398×961). Sidebar region `[class*=sidebarRegion]` with 63 navigation rows (`itemContainer_caf372` → `item_caf372 active_caf372` → `itemContent_caf372`, sub-items `item_e4d939`) across 19 top-level categories + search field (`input[placeholder="Поиск"]`):
  - **Account** (`Учётная запись`): User Profile (`Информация об учётной записи`), Password & Authentication (`Пароль и безопасность`), Account Standing (`Статус учётной записи`), Family Center (`Семейный центр`)
  - Privacy & Safety (`Данные и конфиденциальность`), Message Permissions (`Разрешения для сообщений`), Notifications (`Уведомления`)
  - **Nitro**, Server Boost (`Буст сервера`), Subscriptions (`Подписки`), Gift Inventory (`Склад подарков`), Billing (`Выставление счетов`)
  - **Voice & Video** (`Голос и видео`), Appearance (`Внешний вид`), Accessibility (`Специальные возможности`), System (`Система`), Language & Time (`Язык и время`)
  - Registered Games (`Зарегистрированные игры`), Activity Privacy (`Конфиденциальность активности`), Authorized Apps (`Подключённые приложения`), Developer Mode (`Разработчик`), Log Out (`Выйти`)

### 4.2 "Voice & Video" Tab
- Sections: **Voice** (Input/Output devices + Mic Test), **Input Profile**, **Input Sensitivity**, **Noise Suppression**, **Echo Cancellation / Push to Talk / AGC**, **Camera**, **Streaming**, **Soundboard**, **Advanced**.
- Comboboxes (4, `role="combobox"`): Input device (`Система: Easy Effects Source`), Output device (`Система: Easy Effects Sink`), Camera (`Нет видеоустройств`), **Noise Suppression → "Krisp"** (`aria-label="Noise Suppression"` / `Шумоподавление`).
- **Input Profile** (`data-mana-component="BaseRadioGroup"`, `role="radiogroup"`): Voice Isolation (`Изоляция голоса`), Studio (`Студия`), Custom (`Пользовательский`).
- Toggles: 38 switches (`<input type="checkbox" role="switch" data-react-aria-pressable="true">`).
- Sliders (4, `role="slider"`, class `slider__4c059`, `aria-orientation="horizontal"`): Input Volume (`0–100`, `now=100`), Output Volume (`0–200`, `now=100`), Stream Attenuation (`1–100`, `now=50`, displayed `49%`), Input Sensitivity Threshold (`0–100`, `now≈26.11`).

---

## 5. Component Contracts (12 Components)

1. **Button** (`data-mana-component="button"`)
   - Module: 657718 (`discord.com_assets_web.4d4b2f75fdea5470.js`), CSS 65406 (`button_a22cb0`):
     Variants: primary, secondary, critical-primary, critical-secondary, overlay-primary, overlay-secondary, icon-only, input-accessory, active, expressive, text-button, toggle-icon-button. Sizes: xs (`xs_a22cb0`), sm (`sm_a22cb0`), md (`md_a22cb0`). States: hover/active, disabled, loading (`aria-busy`), hasText (`hasText_a22cb0`), fullWidth (`fullWidth_a22cb0`), rounded. Tokens: `--button-border-radius`, `--button-border-radius-lg`, `--button-danger-background-disabled`, `--button-outline-brand-border-active`, `--button-outline-primary-text`.
2. **TextInput / TextArea** (`input__75098`, `data-mana-component="text-area"`)
   - Module 103557 (`discord.com_assets_05bfdf7077e1fc11.js:1161`), CSS 358983 (`discord.com_assets_348181.c6a72b8c6216ab6c.js:10813`). Single-line and multiline with tag chips (`hasTags`), leading/trailing adornments (`hasLeading`, `hasTrailing`), and combobox (`comboBoxInput__0edde`). Tokens: `--input-background-default`, `--input-icon-default`, `--input-placeholder-text-default`, `--input-border-error-default`, `--input-text-error-default`.
3. **Menu (Mana)**
   - Module 798819 (`discord.com_assets_261000.1b6532d3649c0f52.js:6116`), implementation module 980707. Classes: `menu_c1e9c4`, `flexible_c1e9c4`, `scroller_c1e9c4`, `separator_c1e9c4`. Items: colorDefault, colorDanger, checkbox, radioSelection (`aria-checked`), submenu. Tokens: `--menu-scroller-block-padding`, `--menu-border-width`.
4. **ContextMenu**
   - Opener: Module 837528 (`(0,a.L3)`), primitives Module 442433 (`CONTEXT_MENU_OPEN/CLOSE`), keybind registry Module 696760 (`mod+f` → `FOCUS_SEARCH`), Module 675704 (`mod+k` → QuickSwitcher). Rendered inside popover overlay layer (`layer__59d0d`); item `message-add-reaction` opens compact reaction picker.
5. **Modal** (`data-mana-component="layer-modal"`)
   - Module 521489 (`discord.com_assets_e6c31aeaf72a699a.js`): `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, focus trap (Module 617986: `[data-mana-component="modal"], [role="dialog"][tabindex="-1"]`). Tokens: `--modal-width-small: 442px`, `--modal-width-medium: 602px`, `--modal-width-large: 800px`, `--modal-vertical-padding: 16px`, `--modal-background`, `--modal-footer-background`.
6. **Popover** (`data-mana-component="popover"`)
   - Modules 179657 (`discord.com_assets_267e963ebabea6ad.js:9316`), 43105, 273875. Hosted in `layer__59d0d` / `positionLayer__08434` / `expressionPickerPositionLayer__74017`.
7. **Tooltip**
   - CSS 233680 (`discord.com_assets_348181.c6a72b8c6216ab6c.js:657`): `tooltip_c36707`, `tooltipPointer_c36707`, `tooltipDisablePointerEvents_c36707`, `richTooltipContent__9038b`, and `data-mana-component="mini-premium-tooltip"` (Module 461536).
8. **Switch**
   - React-Aria switch (Module 35556 / Module 479909): `inputProps: {role: "switch", checked}`, `<input type="checkbox" role="switch" data-react-aria-pressable="true">`. Tokens: `--switch-background-selected-default`, `--switch-border-default`, `--switch-border-selected-hover`, `--switch-thumb-background-default`, `--switch-thumb-icon-active`.
9. **Slider**
   - CSS 9761 (`discord.com_assets_500186.e89df1f0566f54ee.js:130`): `slider__4c059`, `role="slider"`, `aria-valuemin`, `aria-valuemax`, `aria-valuenow`, `aria-orientation="horizontal"`, `aria-labelledby`, bar, grabber, disabled, mark, defaultValue, `hasMarks`. Token: `--slider-track-background`.
10. **Avatar**
    - CSS 311582 (`discord.com_assets_113561.4511bb7966772f25.js:259`) & Module 818888: `avatar_fdd9da`, `grayscaleAvatar_fdd9da`, `avatarWrapper_d8a370`, `ring_d8a370`, live `wrapper__44b0c`, `avatar__44b0c`, `avatarStack__44b0c` with SVG status mask cutouts (`32×32` sidebar / `80×80` user popout).
11. **Badge**
    - Module 306091 (`discord.com_assets_880248.92a937f61f4f886c.js:299`), Module 638855 (`badge_e7c728`), Module 198476 (`numberBadge_c7a32c`), Module 672812 (`numberBadge__2ea32`): `numberBadge__463b7`, `base__463b7`, `iconBadge__463b7`, `premiumBadge__463b7`. Tokens: `--badge-secondary-background`, `--badge-expressive-text`, `--badge-epic-border-gradient-end`, `--badge-mythic-background`.
12. **Scroller**
    - CSS 6095 (`discord.com_assets_193433.213ada0bddfee7ee.js`): `scrollerBase_d125d2`, options `auto_d125d2`/`none_d125d2`/`thin_d125d2`, `scrollbarGutterStable_d125d2`, `fade_d125d2`, `content_d125d2`, `customTheme`, virtualized list `scroller_affa7e` + `listItems_affa7e`, menu scroller `scroller_c1e9c4`. Tokens: `--scrollbar-thin-thumb`, `--scrollbar-thin-track`, `--scrollbar-auto-thumb`, `--scrollbar-auto-track`.

---

## 6. Design Tokens Reference (Dark Theme Extract)

| Token | Computed Value |
|---|---|
| `--modal-width-small/medium/large` | 442px / 602px / 800px |
| `--modal-background`, `--modal-footer-background` | hsl(240 6.494% 15.098%) |
| `--modal-vertical-padding` | 16px |
| `--switch-background-selected-default` | hsl(234.935 85.556% 64.706%) |
| `--switch-border-default` | hsl(240 4% 60.784% / 0.2) |
| `--switch-thumb-background-default` | hsl(0 0% 100%) |
| `--input-background-default` | hsl(0 0% 0% / 0.1216) |
| `--input-border-error-default` | hsl(1.905 90% 72.549%) |
| `--button-border-radius(-lg)` | 2147483647px (pill) |
| `--button-danger-background-disabled` | hsl(355.636 64.706% 50%) |
| `--menu-scroller-block-padding` / `--menu-border-width` | 8px / 1px |
| `--scrollbar-thin-thumb` | hsl(234.545 5.473% 39.412%) |
| `--badge-secondary-background` | hsl(240 4% 60.784% / 0.1608) |
| `--text-strong` / `--background-surface-high` | hsl(0 0% 98.431%) / hsl(240 6.494% 15.098%) |

---

## 7. Keyboard Bindings & ARIA Roles Matrix

| Surface / Component | Primary DOM Root & Hierarchy | ARIA Roles & Attributes | Keyboard Bindings |
|---|---|---|---|
| **Channel Sidebar & Guild Rail** | `nav.wrapper_ef3116` → `div.tree_ef3116`; `nav.privateChannels_e6b769` → `div.scroller__99e7c` → `ul.content_d125d2` → `li.channel__972a0` / `a.link__2ea32`; `section.panels__5e434` | `role="tree"`, `role="treeitem"`, `role="group"`, `role="list"`, `role="listitem"`, `role="separator"`, `role="switch"`, `aria-posinset`, `aria-setsize`, `aria-expanded` | `ArrowUp`/`ArrowDown` (roving focus), `ArrowLeft`/`ArrowRight` (collapse/expand category), `Home`/`End`, `Enter`/`Space` (open channel), `Ctrl+K` (QuickSwitcher) |
| **Chat Scroll Virtualization** | `div.scrollerBase_d125d2.scrollbarGutterStable_d125d2` → `div.wrapper_c19a55.cozy_c19a55.zalgo_c19a55` → `div.markup__75297.messageContent_c19a55` | `role="log"`, `role="article"`, `role="separator"`, `aria-live="polite"`, `aria-setsize`, `aria-posinset` | `PageUp`/`PageDown` (page scroll), `Home`/`End` (jump oldest/latest), `ArrowUp` (edit last message), `Escape` (jump to bottom & mark read) |
| **Emoji Picker (`Ctrl+E` & Reaction)** | `div.expressionPickerPositionLayer__74017.layer__59d0d` → `div.emojiPicker_c0e32c` → `div.listItems_affa7e` + `div.listWrapper_c656ac` → `button.emojiItem_fc7141` | `role="dialog"`, `role="tablist"`, `role="tab"`, `aria-selected="true"`, `role="grid"`, `role="row"`, `role="gridcell"` | `Ctrl+E` (toggle picker), `ArrowUp`/`ArrowDown`/`ArrowLeft`/`ArrowRight` (2D grid navigation), `Tab`/`Shift+Tab`, `Enter` (insert emoji), `Escape` (close) |
| **User Popout & Member Roster** | `div.popoutContainer_ce8328.accountPopout__37e49` → `div.outer_c0bea0` → `div.header__5be3e` + `div.body_ce8328`; `aside.membersWrap_c8ffbb` → `div.member__5d473` | `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, `role="img"`, `role="group"`, `role="button"`, `role="listitem"`, `aria-expanded` | `Enter`/`Space` (open popout), `Tab`/`Shift+Tab` (focus trap inside popout), `ArrowUp`/`ArrowDown` (menu/member rows), `Escape` (dismiss popout) |
| **Settings Navigation & Voice Tab** | `div.container_abd9a8` (1398×961) → `div[class*=sidebarRegion]` (`item_caf372`) + `div[class*=contentRegion]` (`slider__4c059`) | `role="tab"`, `role="combobox"`, `role="radiogroup"`, `role="radio"`, `role="switch"`, `role="slider"`, `aria-valuemin`/`max`/`now`, `aria-orientation="horizontal"` | `ArrowUp`/`ArrowDown` (navigate 63 sidebar rows), `ArrowLeft`/`ArrowRight` (adjust slider), `Space`/`Enter` (toggle switch/radio), `Escape` (close settings) |
| **Guild Search & Mana Menus** | `div.editor__25bec` + `div.searchResultsWrap_a98f3b` → `div.filterSection_e4772e` + `div.menu_c1e9c4` + `div.searchResultGroup_a7e67f` | `role="searchbox"`, `role="menu"`, `role="menuitem"`, `role="menuitemradio"`, `aria-checked="true"`, `role="group"` | `Ctrl+F` (focus search), `Ctrl+A` (select all & clear scope chip), `ArrowUp`/`ArrowDown` (menu/results), `Enter` (submit/jump), `Escape` (close) |
