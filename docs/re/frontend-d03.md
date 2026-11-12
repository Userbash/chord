# D-03 — UI Surfaces: Search, Emoji, Members, Settings & Component Contracts

- Environment: Discord Desktop (Chrome 148 / Electron 42, build 1.0.162), CDP `http://127.0.0.1:9223`.
- Methodology: Native CDP automation via HTTP `/json/list` (`Connection: close` → WebSocket; `Runtime/Page/Input`), `getBoundingClientRect` interaction, JPEG screenshots with view clipping.
- Boundaries: Read-only inspection; no messages transmitted, voice connections avoided, user settings left unchanged; `logs/`, `functions/`, and `evidence/` directories untouched.
- Artifact collection in `docs/re/ui8/` (drivers `ui-search4..9.mjs`, `ui-emoji5..9.mjs`, `ui-settings7..13.mjs`, `ui-members6.mjs`).
- CSS variable definitions extracted from live client `ui/tokens.json` (4,795 variables, dark theme).

## 1. Search

### 1.1 Results (Guild Search)
- Text input method: `Ctrl+A` in search bar (clears scope token) followed by keystroke dispatch. Target container: contenteditable `editor__25bec`, results panel `searchResultsWrap_a98f3b`, header title `totalResults`.
- Empirical results: `ProBot` query yields 3,302 results; `Hello` query yields 49,887 results.
- Structure: `searchResultGroup_a7e67f` (aria "channel, category") → `container__80bf8` / `searchResult__80bf8`; message row `wrapper_c19a55 cozy_c19a55 zalgo_c19a55`, `id="search-result-<id>"`, `<svg aria-label="Text">`.
- DOM inspection records 6 visible items (`itemCount=6`), classes: `searchResultGroup_a7e67f`, `container__80bf8`, `searchResult__80bf8`, `message__80bf8`, `wrapper_c19a55`.
- Artifacts: `20-search-results.{json,html,jpg}`, `20a-searchbar.json`, `20a-search-open.jpg`, `20a-search-typed.jpg`, `20e-search-typed-rules.jpg`, `20h-winner.json` (`ctrlA+typeChars` against `/api/v9/guilds/<gid>/messages/search`).

### 1.2 Filters
- Sections `filterSection_e4772e`: **From**, **In**, **Has**, **Mentions**, **Date**, **Author Type**, **Pinned** (7 sections).
- Inputs: `input__75098 comboBoxInput__0edde` with placeholders.
- Buttons: "Clear Filters", "Cancel", "Apply Filters".
- "Has" options (9): image, video, link, file, attachment, audio, poll, sticker, forward.

### 1.3 Sorting
- Menu `menu_c1e9c4 flexible_c1e9c4`: "Newest" (`aria-checked=true`), "Oldest", "Most Relevant" (3 options).

### 1.4 Network Characteristics
- With channel scope chip: `GET /api/v9/guilds/{id}/messages/search?channel_id={cid}&content=ProBot...` returns `{"messages":[],"total_results":0}`.
- Unscoped search: `...messages/search?content=ProBot...` returns `total_results=3302`.

## 2. Emoji

### 2.1 Full Picker (Shortcut `Ctrl+E`)
- Container: `positionLayer__08434 expressionPickerPositionLayer__74017 theme-dark images-dark layer__59d0d`.
- Tabs `[role=tab]`: "GIFs", "Stickers", "Emoji" (`aria-selected=true`).
- Search input with placeholder "Search emoji"; "Add Emoji" button.
- Grid: 50 visible `emojiItem_fc7141`, 17 `lockedEmoji_d982c2` (Nitro gated).
- Category rail (10 categories): `top guild emoji`, `recent`, `guild`, `people`, `nature`, `food`, `activity`, `travel`, `objects`, `symbols`.

### 2.2 Reaction Picker (Context Menu)
- Context menu option `message-add-reaction` triggers compact picker: `emojiPicker_c0e32c` + `bodyWrapper_c0e32c`, `emojiPickerListWrapper_c0e32c`, `listWrapper_c656ac`, `categorySection_c656ac`, 5 rows of `emojiListRow_fc7141`, 60 `emojiItem_fc7141`, 43 `lockedEmoji_d982c2`. Search input and tabs omitted.

## 3. Member Roster (Right Sidebar)

- Container: `aside[class*=membersWrap]` → `membersWrap_c8ffbb hiddenMembers_c8ffbb`.
- Groups: "Server Booster — 2", "Online — 735" (`membersGroup` elements).
- Rows: 35 visible `member__5d473 container__91a9d clickable__91a9d`; contains `avatarStack__44b0c`, name `name__91a9d`, activity status `subText__91a9d`.

## 4. Settings

### 4.1 Navigation Hierarchy
- Trigger: Button with `aria-label="User Settings"` in user panel.
- Root: `container_abd9a8` (1398×961). 63 navigation rows + search field:
  - **Account**: User Profile, Password & Authentication, Account Standing, Family Center
  - Privacy & Safety, Message Permissions, Notifications
  - **Nitro**, Server Boost, Subscriptions, Gift Inventory, Billing
  - **Voice & Video**, Appearance, Accessibility, System, Language & Time
  - Registered Games, Activity Privacy, Authorized Apps, Developer Mode, Log Out

### 4.2 "Voice & Video" Tab
- Sections: **Voice** (Input/Output devices + Mic Test), **Input Profile**, **Input Sensitivity**, **Noise Suppression**, **Echo Cancellation / Push to Talk / AGC**, **Camera**, **Streaming**, **Soundboard**, **Advanced**.
- Comboboxes: Input device, Output device, Camera, **Noise Suppression → "Krisp"** (`aria-label="Noise Suppression"`).
- **Input Profile** (`role=radiogroup`): Voice Isolation, Studio, Custom.
- Toggles: 38 switches (`role="switch"`).
- Sliders (`role="slider"`, class `slider__4c059`): Input Volume (0–100), Output Volume (0–200), Stream Attenuation (1–100).

## 5. Component Contracts (12 Components)

1. **Button** (`data-mana-component="button"`)
   - Module: 657718 (`discord.com_assets_web.4d4b2f75fdea5470.js`):
     Variants: primary, secondary, critical-primary, critical-secondary, overlay-primary, overlay-secondary, icon-only, input-accessory, active, expressive. Sizes: xs, sm, md. Tokens: `--button-border-radius`, `--button-outline-brand-border-active`.
2. **TextInput / TextArea** (`input__75098`, `data-mana-component="text-area"`)
   - Module 103557 (`discord.com_assets_05bfdf7077e1fc11.js:1161`). Single-line and multiline with tag chips and adornments. Tokens: `--input-background-default`, `--input-border-error-default`.
3. **Menu (Mana)**
   - Module 798819 (`discord.com_assets_261000.1b6532d3649c0f52.js:6116`). Items: colorDefault, colorDanger, checkbox, radioSelection, submenu; separator `separator_c1e9c4`.
4. **ContextMenu**
   - Opener: Module 837528. Handlers: `CONTEXT_MENU_OPEN/CLOSE`. Rendered inside popover overlay layer.
5. **Modal** (`data-mana-component="layer-modal"`)
   - Module 521489: `role="dialog"`, `aria-labelledby`, focus trap (Module 617986). Tokens: `--modal-width-small: 442px`, `--modal-width-medium: 602px`, `--modal-width-large: 800px`, `--modal-background`.
6. **Popover** (`data-mana-component="popover"`)
   - Hosted in `layer__59d0d` / `positionLayer__*`.
7. **Tooltip**
   - CSS 233680: `tooltip_c36707`, `tooltipPointer_c36707`, mini-premium-tooltip.
8. **Switch**
   - React-Aria switch: `inputProps: {role: "switch", checked}`. Tokens: `--switch-background-selected-default`, `--switch-border-default`, `--switch-thumb-background-default`.
9. **Slider**
   - CSS 9761: `slider__4c059`, bar, grabber, disabled, mark, defaultValue. Token: `--slider-track-background`.
10. **Avatar**
    - CSS 311582: `avatar_fdd9da`, `avatarWrapper_d8a370`, `ring_d8a370`.
11. **Badge**
    - Module 306091: `numberBadge__463b7`, `iconBadge__463b7`, `premiumBadge__463b7`. Tokens: `--badge-secondary-background`, `--badge-expressive-text`.
12. **Scroller**
    - CSS 6095: `scrollerBase_d125d2`, options auto/none/thin, `scrollbarGutterStable`, `customTheme`. Tokens: `--scrollbar-thin-thumb`, `--scrollbar-auto-thumb`.

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
