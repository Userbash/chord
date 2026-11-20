# Wave RW-53: Frontend & UI Modernization (10 Parallel Worker Streams)

## Executive Summary
This document specifies the 10 non-overlapping worker task streams for the complete modernization, cleanroom design alignment, and library integration of the `discord-lab` / `discord-gpui` webview frontend.
Each stream is assigned a strict write-scope, cleanroom design reference from `/home/sanya/re-deep`, contract tests, and model complexity tier.

---

## 1. Department Breakdown & 10 Worker Matrix

| Worker | Role | Department | Write-Scope | Libraries & Bridges | Model Tier & Effort |
|---|---|---|---|---|---|
| **W01** | Voice Call Stage & Video Demo Grid | `G2/G5 Voice Frontend` | `ui/features/voice-call-stage.js`, `voice-dm-call-stage.js`, `voice-call-orchestrator.js` | `@preact/signals-core`, `auto-ipc-bridge.js` | Tier M: `gemini-4-hydrogen-medium` (`effort: medium`) |
| **W02** | Virtualized Chat History & Scroller | `G2 Frontend` | `ui/features/chat.js`, `virtual-list.js`, `virtualizer.js` | `@tanstack/virtual-core`, `@preact/signals-core` | Tier M: `gemini-4-hydrogen-medium` (`effort: medium`) |
| **W03** | Discord Markdown & Codeblock Engine | `G2 Frontend` | `ui/features/markdown.js`, `markdown-helper.js` | Modular AST tokenizer, safe DOM construction | Tier M: `gemini-4-hydrogen-medium` (`effort: medium`) |
| **W04** | Guild Member List & Role Groups | `G2 Frontend` | `ui/features/member-list.js`, `members.js` | `@tanstack/virtual-core`, `@floating-ui/dom` | Tier M: `gemini-4-hydrogen-medium` (`effort: medium`) |
| **W05** | Chat Composer, Autocomplete & Files | `G2 Frontend` | `ui/features/composer.js`, `composer-autocomplete.js`, `attachments.js` | `@floating-ui/dom`, `tinykeys`, `auto-ipc-bridge.js` | Tier M: `gemini-4-hydrogen-medium` (`effort: medium`) |
| **W06** | Expression Picker, Emojis & Reactions | `G2 Frontend` | `ui/features/expression-picker.js`, `live-emoji-picker.js`, `reactions.js`, `reactions-ui.js` | `@floating-ui/dom`, `@tanstack/virtual-core`, Cleanroom SVGs | Tier M: `gemini-4-hydrogen-medium` (`effort: medium`) |
| **W07** | Cleanroom User Profile Popout & Modal | `G2/G3 Frontend & Design`| `ui/features/user-popout.js`, `profile-modal.js`, `styles/user-popout.css` | `@floating-ui/dom`, `tokens-cleanroom.css` | Tier S: `gemini-4-hydrogen-low` (`effort: low`) |
| **W08** | Channel Navigation & Server Guild Tree | `G2 Frontend` | `ui/features/channel-tree.js`, `channel-tree-controller.js`, `channel-node-renderer.js` | `@preact/signals-core`, `tokens-cleanroom.css` | Tier M: `gemini-4-hydrogen-medium` (`effort: medium`) |
| **W09** | Cleanroom Settings & Preferences Modal | `G2 Frontend` | `ui/features/settings-modal.js`, `settings.js`, `voice-devices-settings.js` | `tinykeys`, `auto-ipc-bridge.js` | Tier M: `gemini-4-hydrogen-medium` (`effort: medium`) |
| **W10** | Async Typed IPC Bridge & State Router | `G1/G2 BFF Glue` | `ui/features/auto-ipc-bridge.js`, `discord-native-bridge.js` | `scripts/generate-ipc-bridges.mjs`, Event Router | Tier L: `gemini-4-argon-medium` (`effort: medium`) |

---

## 2. Detailed Task Specifications

### W01: Voice Call Stage & Video Demo Grid
- **DoD:** 
  1. Live voice stage with large avatars, speaking ring indicators (`data-speaking="true"`), and full video stream tile with "В ЭФИРЕ" badge and "720p 60 кадров в секунду".
  2. Floating control dock (mic chevron, camera, screenshare, soundboard, disconnect) positioned at the bottom of the call view.
  3. Ensure stage never disappears when switching between DM channels or when viewing stream demos (as reported in user screenshots).
- **Targeted Test:** `node --test tests/unit/voice-call-stage.test.mjs`

### W02: Virtualized Chat History & Scroller
- **DoD:**
  1. Unified scroller powered by `@tanstack/virtual-core` with dynamic height measurement (`measureElement`).
  2. Sticky date separators ("18 ноября 2026 г.") and unread pills ("Новые сообщения").
  3. Smooth scrolling for 100,000+ messages without DOM bloat (DOM pool maintains < 35 elements).
- **Targeted Test:** `node --test tests/unit/esm-libraries-integration.test.mjs`

### W03: Discord Markdown & Codeblock Engine
- **DoD:**
  1. Full parser for Discord formatting: bold (`**`), italic (`*`/`_`), underline (`__`), strikethrough (`~~`), code (`\``), multi-line codeblocks with syntax highlighting and copy button (`\`\`\`lang`).
  2. Mentions (`<@123>`), channels (`<#456>`), custom emojis (`<:name:789>`), timestamp tags (`<t:timestamp:R>`), spoilers (`||secret||`).
  3. Zero regex catastrophic backtracking, safe DOM tree construction without `eval` or unsafe HTML.
- **Targeted Test:** `node --test tests/unit/markdown-contract.test.mjs`

### W04: Guild Member List & Role Groups
- **DoD:**
  1. Virtualized list of members grouped by roles (e.g. "Онлайн — 14", "Оффлайн — 128") handling 25,000+ users.
  2. Avatar badges with presence statuses (online, idle, dnd, offline, mobile, streaming).
  3. Hover cards and right-click context menu triggers tethered via `@floating-ui/dom`.
- **Targeted Test:** `node --test tests/unit/member-list-virtual.test.mjs`

### W05: Chat Composer, Autocomplete & Files
- **DoD:**
  1. Auto-growing textarea with `tinykeys` keyboard shortcuts (Enter to send, Shift+Enter for newline, Up arrow to edit last message).
  2. Autocomplete popovers anchored via `@floating-ui/dom` for slash commands (`/`), user mentions (`@`), and emoji shortcodes (`:`).
  3. Drag-and-drop file attachment queue with thumbnail previews and 25 MB validation.
- **Targeted Test:** `node --test tests/unit/composer-autocomplete.test.mjs`

### W06: Expression Picker, Emojis & Reactions
- **DoD:**
  1. Cleanroom Discord emoji/GIF/sticker picker with category scrolling, live search filter, and skin tone selector.
  2. Quick reaction bar on message hover (top 5 frequent emojis + add reaction button).
  3. Reactive emoji reaction counters with animated pulse on click.
- **Targeted Test:** `node --test tests/unit/emoji-reactions-contract.test.mjs`

### W07: Cleanroom User Profile Popout & Modal
- **DoD:**
  1. Cleanroom user profile popout matching `/home/sanya/re-deep` styling and `tokens-cleanroom.css`.
  2. Banner with custom color/image, avatar with decoration and status, display name, handle (`@username`), badges (Nitro, Booster, HypeSquad).
  3. Bio with markdown, activity card ("В игре", "Слушает Spotify"), mutual servers/friends tabs.
- **Targeted Test:** `node --test tests/unit/user-popout.test.mjs`

### W08: Channel Navigation & Server Guild Tree
- **DoD:**
  1. Left-most guild navigation bar (server icons, home direct messages button, guild folders with expand/collapse, unread indicators).
  2. Channel sidebar (collapsible categories, voice channel speaker badges, stage channels, text channels with unread/mention counters).
  3. Signals-based active channel routing via `@preact/signals-core`.
- **Targeted Test:** `node --test tests/unit/channel-navigation.test.mjs`

### W09: Cleanroom Settings & Preferences Modal
- **DoD:**
  1. Cleanroom settings dialog with left category list (Учётная запись, Профиль, Голос и видео, Внешний вид, Горячие клавиши).
  2. Audio settings tab with input/output device dropdowns and real-time microphone test volume meter.
  3. Appearance tab with Dark / Light / Midnight themes and font scaling. ESC key to exit.
- **Targeted Test:** `node --test tests/unit/settings-modal.test.mjs`

### W10: Async Typed IPC Bridge & State Router
- **DoD:**
  1. Auto-generated typed async wrappers (`auto-ipc-bridge.js`) for all 81 Tauri commands.
  2. Event-driven push notifications (`gateway_event`, `voice_state_update`, `message_create`).
  3. Reconnection logic with exponential backoff and offline queueing.
- **Targeted Test:** `node --test tests/unit/auto-ipc-bridge.test.mjs`
