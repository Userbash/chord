# UI Feature Test Coverage Ledger

> **Date:** 2026-11-16 12:26:11 UTC
> **Total UI Feature Modules:** 144
> **Covered Modules:** 96 (66.7%)
> **Gaps (Pending Coverage):** 48 (33.3%)
> **Total Unit Test Suites:** 244

---

## 1. Summary by Status

| Status | Count | Percentage |
|---|---|---|
| ✅ **Covered by Unit/Contract Tests** | 96 | 66.7% |
| ⚪ **Gaps (Awaiting Direct Tests)** | 48 | 33.3% |
| **Total** | **144** | **100.0%** |

---

## 2. Covered Feature Modules (96)

| # | Feature Module (`rust/gui-spike/ui/features/`) | Test Suite (`tests/unit/`) |
|---|---|---|
| 1 | `actions.js` | [`chat-reactions.test.mjs`](file:///home/sanya/orca/project/tests/unit/chat-reactions.test.mjs) |
| 2 | `advanced-settings.js` | [`advanced-settings.test.mjs`](file:///home/sanya/orca/project/tests/unit/advanced-settings.test.mjs) |
| 3 | `appearance-settings.js` | [`appearance-settings.test.mjs`](file:///home/sanya/orca/project/tests/unit/appearance-settings.test.mjs) |
| 4 | `attachment-helper.js` | [`attachment-helper.test.mjs`](file:///home/sanya/orca/project/tests/unit/attachment-helper.test.mjs) |
| 5 | `attachment-validator.js` | [`attachment-validator.test.mjs`](file:///home/sanya/orca/project/tests/unit/attachment-validator.test.mjs) |
| 6 | `attachments.js` | [`attachments-render.test.mjs`](file:///home/sanya/orca/project/tests/unit/attachments-render.test.mjs) |
| 7 | `audit-log.js` | [`audit-log-parser-1008a.test.mjs`](file:///home/sanya/orca/project/tests/unit/audit-log-parser-1008a.test.mjs) |
| 8 | `auth.js` | [`gui-spike-auth.test.mjs`](file:///home/sanya/orca/project/tests/unit/gui-spike-auth.test.mjs) |
| 9 | `automod-editor.js` | [`automod-editor.test.mjs`](file:///home/sanya/orca/project/tests/unit/automod-editor.test.mjs) |
| 10 | `captcha-modal.js` | [`captcha-modal-interactive.test.mjs`](file:///home/sanya/orca/project/tests/unit/captcha-modal-interactive.test.mjs) |
| 11 | `channel-gate.js` | [`channel-gate.test.mjs`](file:///home/sanya/orca/project/tests/unit/channel-gate.test.mjs) |
| 12 | `channel-tree-controller.js` | [`channel-tree-controller.test.mjs`](file:///home/sanya/orca/project/tests/unit/channel-tree-controller.test.mjs) |
| 13 | `channel-tree.js` | [`channel-tree-controller.test.mjs`](file:///home/sanya/orca/project/tests/unit/channel-tree-controller.test.mjs) |
| 14 | `chat.js` | [`chat-reactions.test.mjs`](file:///home/sanya/orca/project/tests/unit/chat-reactions.test.mjs) |
| 15 | `composer-autocomplete.js` | [`composer-autocomplete.test.mjs`](file:///home/sanya/orca/project/tests/unit/composer-autocomplete.test.mjs) |
| 16 | `composer.js` | [`composer-autocomplete.test.mjs`](file:///home/sanya/orca/project/tests/unit/composer-autocomplete.test.mjs) |
| 17 | `context-menu-router.js` | [`context-menu-router.test.mjs`](file:///home/sanya/orca/project/tests/unit/context-menu-router.test.mjs) |
| 18 | `context-menu.js` | [`context-menu-router.test.mjs`](file:///home/sanya/orca/project/tests/unit/context-menu-router.test.mjs) |
| 19 | `create-guild.js` | [`create-guild-wizard.test.mjs`](file:///home/sanya/orca/project/tests/unit/create-guild-wizard.test.mjs) |
| 20 | `custom-themes.js` | [`custom-themes-985.test.mjs`](file:///home/sanya/orca/project/tests/unit/custom-themes-985.test.mjs) |
| 21 | `dm-friends.js` | [`dm-friends-ui.test.mjs`](file:///home/sanya/orca/project/tests/unit/dm-friends-ui.test.mjs) |
| 22 | `dom.js` | [`domains.test.mjs`](file:///home/sanya/orca/project/tests/unit/domains.test.mjs) |
| 23 | `embed-formatter.js` | [`embed-formatter.test.mjs`](file:///home/sanya/orca/project/tests/unit/embed-formatter.test.mjs) |
| 24 | `emoji.js` | [`custom-emojis-stickers-1005.test.mjs`](file:///home/sanya/orca/project/tests/unit/custom-emojis-stickers-1005.test.mjs) |
| 25 | `emojis.js` | [`custom-emojis-stickers-1005.test.mjs`](file:///home/sanya/orca/project/tests/unit/custom-emojis-stickers-1005.test.mjs) |
| 26 | `event-banner.js` | [`event-banner.test.mjs`](file:///home/sanya/orca/project/tests/unit/event-banner.test.mjs) |
| 27 | `events-modal.js` | [`events-modal.test.mjs`](file:///home/sanya/orca/project/tests/unit/events-modal.test.mjs) |
| 28 | `expression-picker.js` | [`expression-picker.test.mjs`](file:///home/sanya/orca/project/tests/unit/expression-picker.test.mjs) |
| 29 | `forum-filter.js` | [`forum-filter.test.mjs`](file:///home/sanya/orca/project/tests/unit/forum-filter.test.mjs) |
| 30 | `forum-view.js` | [`forum-view-1001b.test.mjs`](file:///home/sanya/orca/project/tests/unit/forum-view-1001b.test.mjs) |
| 31 | `friends-home.js` | [`friends-home.test.mjs`](file:///home/sanya/orca/project/tests/unit/friends-home.test.mjs) |
| 32 | `gift-modal.js` | [`gift-modal.test.mjs`](file:///home/sanya/orca/project/tests/unit/gift-modal.test.mjs) |
| 33 | `guild-folders.js` | [`guild-folders.test.mjs`](file:///home/sanya/orca/project/tests/unit/guild-folders.test.mjs) |
| 34 | `guild-modals.js` | [`guild-modals.test.mjs`](file:///home/sanya/orca/project/tests/unit/guild-modals.test.mjs) |
| 35 | `inbox.js` | [`inbox-panel.test.mjs`](file:///home/sanya/orca/project/tests/unit/inbox-panel.test.mjs) |
| 36 | `ingame-overlay.js` | [`ingame-overlay-988.test.mjs`](file:///home/sanya/orca/project/tests/unit/ingame-overlay-988.test.mjs) |
| 37 | `invite-modal.js` | [`invite-modal.test.mjs`](file:///home/sanya/orca/project/tests/unit/invite-modal.test.mjs) |
| 38 | `invite-parser.js` | [`invite-parser.test.mjs`](file:///home/sanya/orca/project/tests/unit/invite-parser.test.mjs) |
| 39 | `join-server.js` | [`join-server.test.mjs`](file:///home/sanya/orca/project/tests/unit/join-server.test.mjs) |
| 40 | `keybinds.js` | [`keybinds.test.mjs`](file:///home/sanya/orca/project/tests/unit/keybinds.test.mjs) |
| 41 | `locale-switcher.js` | [`locale-switcher.test.mjs`](file:///home/sanya/orca/project/tests/unit/locale-switcher.test.mjs) |
| 42 | `markdown-helper.js` | [`markdown-helper.test.mjs`](file:///home/sanya/orca/project/tests/unit/markdown-helper.test.mjs) |
| 43 | `markdown.js` | [`markdown-helper.test.mjs`](file:///home/sanya/orca/project/tests/unit/markdown-helper.test.mjs) |
| 44 | `member-list.js` | [`member-list.test.mjs`](file:///home/sanya/orca/project/tests/unit/member-list.test.mjs) |
| 45 | `members.js` | [`members-subscriptions.test.mjs`](file:///home/sanya/orca/project/tests/unit/members-subscriptions.test.mjs) |
| 46 | `mentions.js` | [`markdown-mentions.test.mjs`](file:///home/sanya/orca/project/tests/unit/markdown-mentions.test.mjs) |
| 47 | `message-actions.js` | [`message-actions.test.mjs`](file:///home/sanya/orca/project/tests/unit/message-actions.test.mjs) |
| 48 | `message-components.js` | [`message-components.test.mjs`](file:///home/sanya/orca/project/tests/unit/message-components.test.mjs) |
| 49 | `message-media-polls.js` | [`message-media-polls.test.mjs`](file:///home/sanya/orca/project/tests/unit/message-media-polls.test.mjs) |
| 50 | `message-rendering.js` | [`message-rendering-and-emoji.test.mjs`](file:///home/sanya/orca/project/tests/unit/message-rendering-and-emoji.test.mjs) |
| 51 | `metrics.js` | [`rtc-metrics-1011a.test.mjs`](file:///home/sanya/orca/project/tests/unit/rtc-metrics-1011a.test.mjs) |
| 52 | `modal-router.js` | [`modal-router.test.mjs`](file:///home/sanya/orca/project/tests/unit/modal-router.test.mjs) |
| 53 | `notifications.js` | [`notifications-ui.test.mjs`](file:///home/sanya/orca/project/tests/unit/notifications-ui.test.mjs) |
| 54 | `pins-threads.js` | [`bff-pins-threads-contract.test.mjs`](file:///home/sanya/orca/project/tests/unit/bff-pins-threads-contract.test.mjs) |
| 55 | `plural-format.js` | [`plural-format.test.mjs`](file:///home/sanya/orca/project/tests/unit/plural-format.test.mjs) |
| 56 | `poll-card.js` | [`poll-card.test.mjs`](file:///home/sanya/orca/project/tests/unit/poll-card.test.mjs) |
| 57 | `poll-progress.js` | [`poll-progress.test.mjs`](file:///home/sanya/orca/project/tests/unit/poll-progress.test.mjs) |
| 58 | `presence-activity.js` | [`presence-activity.test.mjs`](file:///home/sanya/orca/project/tests/unit/presence-activity.test.mjs) |
| 59 | `profile-modal.js` | [`profile-modal.test.mjs`](file:///home/sanya/orca/project/tests/unit/profile-modal.test.mjs) |
| 60 | `qr-auth-modal.js` | [`qr-auth-modal.test.mjs`](file:///home/sanya/orca/project/tests/unit/qr-auth-modal.test.mjs) |
| 61 | `quick-switcher.js` | [`quick-switcher.test.mjs`](file:///home/sanya/orca/project/tests/unit/quick-switcher.test.mjs) |
| 62 | `reaction-verification.js` | [`reaction-verification.test.mjs`](file:///home/sanya/orca/project/tests/unit/reaction-verification.test.mjs) |
| 63 | `reactions-ui.js` | [`reactions-ui.test.mjs`](file:///home/sanya/orca/project/tests/unit/reactions-ui.test.mjs) |
| 64 | `reactions.js` | [`chat-reactions.test.mjs`](file:///home/sanya/orca/project/tests/unit/chat-reactions.test.mjs) |
| 65 | `reactive-store.js` | [`reactive-store.test.mjs`](file:///home/sanya/orca/project/tests/unit/reactive-store.test.mjs) |
| 66 | `role-editor.js` | [`role-editor.test.mjs`](file:///home/sanya/orca/project/tests/unit/role-editor.test.mjs) |
| 67 | `role-permissions.js` | [`role-permissions.test.mjs`](file:///home/sanya/orca/project/tests/unit/role-permissions.test.mjs) |
| 68 | `rules-screening-modal.js` | [`rules-screening-modal.test.mjs`](file:///home/sanya/orca/project/tests/unit/rules-screening-modal.test.mjs) |
| 69 | `screen-share-dialog.js` | [`screen-share-dialog.test.mjs`](file:///home/sanya/orca/project/tests/unit/screen-share-dialog.test.mjs) |
| 70 | `search-panel.js` | [`search-panel.test.mjs`](file:///home/sanya/orca/project/tests/unit/search-panel.test.mjs) |
| 71 | `search-popover.js` | [`search-popover-ui.test.mjs`](file:///home/sanya/orca/project/tests/unit/search-popover-ui.test.mjs) |
| 72 | `search.js` | [`search-panel.test.mjs`](file:///home/sanya/orca/project/tests/unit/search-panel.test.mjs) |
| 73 | `settings-modal.js` | [`settings-modal.test.mjs`](file:///home/sanya/orca/project/tests/unit/settings-modal.test.mjs) |
| 74 | `settings.js` | [`advanced-settings.test.mjs`](file:///home/sanya/orca/project/tests/unit/advanced-settings.test.mjs) |
| 75 | `shell-bridge.js` | [`shell-bridge.test.mjs`](file:///home/sanya/orca/project/tests/unit/shell-bridge.test.mjs) |
| 76 | `shortcuts.js` | [`shortcuts.test.mjs`](file:///home/sanya/orca/project/tests/unit/shortcuts.test.mjs) |
| 77 | `slowmode.js` | [`slowmode-contract.test.mjs`](file:///home/sanya/orca/project/tests/unit/slowmode-contract.test.mjs) |
| 78 | `stage-channel.js` | [`stage-channel.test.mjs`](file:///home/sanya/orca/project/tests/unit/stage-channel.test.mjs) |
| 79 | `stream-viewer-ui.js` | [`stream-viewer-ui.test.mjs`](file:///home/sanya/orca/project/tests/unit/stream-viewer-ui.test.mjs) |
| 80 | `stream-viewer.js` | [`stream-viewer-ui.test.mjs`](file:///home/sanya/orca/project/tests/unit/stream-viewer-ui.test.mjs) |
| 81 | `svg-icons.js` | [`svg-icons.test.mjs`](file:///home/sanya/orca/project/tests/unit/svg-icons.test.mjs) |
| 82 | `threads-drawer.js` | [`threads-drawer.test.mjs`](file:///home/sanya/orca/project/tests/unit/threads-drawer.test.mjs) |
| 83 | `token-map.js` | [`token-map.test.mjs`](file:///home/sanya/orca/project/tests/unit/token-map.test.mjs) |
| 84 | `tooltip.js` | [`tooltip.test.mjs`](file:///home/sanya/orca/project/tests/unit/tooltip.test.mjs) |
| 85 | `typing-indicator.js` | [`typing-indicator.test.mjs`](file:///home/sanya/orca/project/tests/unit/typing-indicator.test.mjs) |
| 86 | `unread-bar.js` | [`unread-bar.test.mjs`](file:///home/sanya/orca/project/tests/unit/unread-bar.test.mjs) |
| 87 | `user-popout.js` | [`user-popout.test.mjs`](file:///home/sanya/orca/project/tests/unit/user-popout.test.mjs) |
| 88 | `voice-call-orchestrator.js` | [`voice-call-orchestrator.test.mjs`](file:///home/sanya/orca/project/tests/unit/voice-call-orchestrator.test.mjs) |
| 89 | `voice-devices-settings.js` | [`voice-devices-settings.test.mjs`](file:///home/sanya/orca/project/tests/unit/voice-devices-settings.test.mjs) |
| 90 | `voice-grid.js` | [`voice-grid.test.mjs`](file:///home/sanya/orca/project/tests/unit/voice-grid.test.mjs) |
| 91 | `voice-input-mode.js` | [`voice-input-mode.test.mjs`](file:///home/sanya/orca/project/tests/unit/voice-input-mode.test.mjs) |
| 92 | `voice-quality.js` | [`voice-quality.test.mjs`](file:///home/sanya/orca/project/tests/unit/voice-quality.test.mjs) |
| 93 | `voice-stage-preview.js` | [`voice-stage-preview.test.mjs`](file:///home/sanya/orca/project/tests/unit/voice-stage-preview.test.mjs) |
| 94 | `voice-stage-ui.js` | [`voice-stage-ui.test.mjs`](file:///home/sanya/orca/project/tests/unit/voice-stage-ui.test.mjs) |
| 95 | `voice-volume.js` | [`voice-volume.test.mjs`](file:///home/sanya/orca/project/tests/unit/voice-volume.test.mjs) |
| 96 | `voice.js` | [`afk-voice-status.test.mjs`](file:///home/sanya/orca/project/tests/unit/afk-voice-status.test.mjs) |

---

## 3. Uncovered Feature Gaps (48)

> To generate a test scaffold for any gap below, run:
> `npm run test:gen -- <feature-name>`

| # | Feature Module | Action Command |
|---|---|---|
| 1 | `app-shell-wiring.js` | `npm run test:gen -- app-shell-wiring.js` |
| 2 | `channel-node-renderer.js` | `npm run test:gen -- channel-node-renderer.js` |
| 3 | `channel-switch-permissions.js` | `npm run test:gen -- channel-switch-permissions.js` |
| 4 | `chat-helpers-bridge.js` | `npm run test:gen -- chat-helpers-bridge.js` |
| 5 | `composer-popovers-bridge.js` | `npm run test:gen -- composer-popovers-bridge.js` |
| 6 | `discord-native-bridge.js` | `npm run test:gen -- discord-native-bridge.js` |
| 7 | `dispatch-bridge.js` | `npm run test:gen -- dispatch-bridge.js` |
| 8 | `dispatch-model.js` | `npm run test:gen -- dispatch-model.js` |
| 9 | `dispatch-view.js` | `npm run test:gen -- dispatch-view.js` |
| 10 | `embeds.js` | `npm run test:gen -- embeds.js` |
| 11 | `errors.js` | `npm run test:gen -- errors.js` |
| 12 | `expressions-manager.js` | `npm run test:gen -- expressions-manager.js` |
| 13 | `floating-popover.js` | `npm run test:gen -- floating-popover.js` |
| 14 | `guild-boost.js` | `npm run test:gen -- guild-boost.js` |
| 15 | `ipc-bindings.js` | `npm run test:gen -- ipc-bindings.js` |
| 16 | `live-emoji-picker.js` | `npm run test:gen -- live-emoji-picker.js` |
| 17 | `live-settings-enhancer.js` | `npm run test:gen -- live-settings-enhancer.js` |
| 18 | `message-interaction-controller.js` | `npm run test:gen -- message-interaction-controller.js` |
| 19 | `message-toolbar.js` | `npm run test:gen -- message-toolbar.js` |
| 20 | `moderation-panel.js` | `npm run test:gen -- moderation-panel.js` |
| 21 | `onboarding-wizard.js` | `npm run test:gen -- onboarding-wizard.js` |
| 22 | `pins-threads-wiring.js` | `npm run test:gen -- pins-threads-wiring.js` |
| 23 | `reactive-components.js` | `npm run test:gen -- reactive-components.js` |
| 24 | `rows.js` | `npm run test:gen -- rows.js` |
| 25 | `rtc-quality-status-bar.js` | `npm run test:gen -- rtc-quality-status-bar.js` |
| 26 | `screens.js` | `npm run test:gen -- screens.js` |
| 27 | `search-overlay.js` | `npm run test:gen -- search-overlay.js` |
| 28 | `shell-controllers-registry.js` | `npm run test:gen -- shell-controllers-registry.js` |
| 29 | `slash-command-picker.js` | `npm run test:gen -- slash-command-picker.js` |
| 30 | `soundboard-picker.js` | `npm run test:gen -- soundboard-picker.js` |
| 31 | `stage-request-queue.js` | `npm run test:gen -- stage-request-queue.js` |
| 32 | `thread-badge.js` | `npm run test:gen -- thread-badge.js` |
| 33 | `virtual-list.js` | `npm run test:gen -- virtual-list.js` |
| 34 | `virtualizer.js` | `npm run test:gen -- virtualizer.js` |
| 35 | `voice-connection-controller.js` | `npm run test:gen -- voice-connection-controller.js` |
| 36 | `voice-diagnostics.js` | `npm run test:gen -- voice-diagnostics.js` |
| 37 | `voice-dm-call-stage.js` | `npm run test:gen -- voice-dm-call-stage.js` |
| 38 | `voice-invite-and-devices.js` | `npm run test:gen -- voice-invite-and-devices.js` |
| 39 | `voice-participant-menu.js` | `npm run test:gen -- voice-participant-menu.js` |
| 40 | `voice-popouts-controller.js` | `npm run test:gen -- voice-popouts-controller.js` |
| 41 | `voice-sound-effects.js` | `npm run test:gen -- voice-sound-effects.js` |
| 42 | `voice-stage-renderer.js` | `npm run test:gen -- voice-stage-renderer.js` |
| 43 | `voice-stage-viewer-controls.js` | `npm run test:gen -- voice-stage-viewer-controls.js` |
| 44 | `voice-stage-viewer.js` | `npm run test:gen -- voice-stage-viewer.js` |
| 45 | `voice-state-sync.js` | `npm run test:gen -- voice-state-sync.js` |
| 46 | `voice-stream-controller.js` | `npm run test:gen -- voice-stream-controller.js` |
| 47 | `voice-stream-menus.js` | `npm run test:gen -- voice-stream-menus.js` |
| 48 | `webhooks-editor.js` | `npm run test:gen -- webhooks-editor.js` |
