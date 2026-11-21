# TDD Implementation Plan: `discord-lab` — Strict Test-Driven CLI & Client

Plan Date: 2026-10-18. Updated: 2026-10-19.  
Operating process (roles, waves, quality gates, spec/report templates): `PROCESS.md`; agent guidelines: `AGENTS.md`.  
Rust client specifications: `docs/rust-client/` (`README.md`, `ARCHITECTURE.md`, `PLAN.md`).

Current Status: All core and native layers accepted; zero test failures across test suites. Development proceeds strictly via Test-Driven Development (red → green → refactor).

---

## 1. Product Concept and Principles

The **`discord-lab`** CLI application (package `discord-function-lab`) unifies protocol research, diagnostic tools, and native Rust client integration into a structured, library-backed CLI suite.

Core Principles:
1. **Zero-Build Execution:** The CLI runs directly on modern Node.js using ESM (`.mjs`) and JSDoc type annotations; `tsc --noEmit` validates type safety.
2. **Strict TDD:** No production logic is written without a preceding red test proving the requirement. Sequence: Test → Red → Minimal Implementation → Green → Refactor.
3. **Multi-Tier Testing Matrix:** Unit, smoke, meta, and E2E suites driven by the native `node:test` runner with zero third-party testing dependencies.
4. **Tests as Contracts:** Public function signatures, IPC wire schemas, and data structures are codified as immutable test assertions. Tests are never modified simply to pass broken implementations.
5. **Progressive Parity:** Legacy automation scripts in `scripts/*` remain operational while `src/` modules incrementally replace them across milestones.

---

## 2. Target Directory Structure

```text
bin/discord-lab.mjs        Main CLI entry point (#!/usr/bin/env node): thin run(argv) -> exit code

src/core/                  Pure domain core: zero I/O, deterministic functions
  types.d.ts               Shared TypeScript definitions
  args.mjs                 Argument parsing: --key value / --key=value / --flag / --module
  csv.mjs                  safeName, CSV escaping routines
  paths.mjs                Project paths, logs/, functions/, evidence/screens/, screenshot slugs
  processes.mjs            isDiscordProcess / filterDiscordProcesses
  inventory.mjs            CSV and JSON symbol inventory generation
  calls.mjs                Call record processing: isCallPayload, callLine, parseCallLog
  hook-io.mjs              openCallLog, waitForStop — log stream writers
  report/nm.mjs            parseNmPosix, staticFunctionsCsv
  report/domains.mjs       classifyName, domainCsv, formatDomainSummary
  cdp/client.mjs           CDP client: resolveEndpoint, listTargets, evaluate
  ui/profile.mjs           topFunctions: CPU profile analysis
  ui/blacklist.mjs         isBlacklisted: disallowed UI items
  ui/walk-log.mjs          parseVisited, buildWalkRecord, consoleIssues, shouldBail
  ui/engine.mjs            UI walk step: click -> profile -> screenshot -> log

src/cli/                   CLI commands and orchestration (pids, inventory, hook, summarize)
  run.mjs                  Command router, help/version, exit codes

src/adapters/              External system integration adapters
  cxxfilt.mjs              Symbol demangling via c++filt
  frida.mjs                Frida device and script injection management

tests/
  unit/                    Fast isolated unit tests (zero network/disk I/O)
  meta/                    Meta-tests: plan sync, fixtures tracking, CLI docs alignment
  smoke/                   Real CLI process execution
  e2e/                     Headless browser testing
  helpers/                 FakeWebSocket, temporary directory helpers
  fixtures/                Curated test trace fixtures (MD5 provenance)
```

---

## 3. Execution Commands

```bash
npm test              # Run full test matrix: unit + meta + smoke + e2e
npm run test:unit     # Run unit tests only
npm run test:meta     # Run meta-tests: plan sync §8 <-> tests <-> exports
npm run test:smoke    # Run CLI smoke tests
npm run test:e2e      # Run CDP E2E tests against headless browser
npm run test:coverage # Check code coverage against thresholds (90/80/90)
npm run typecheck     # Run tsc type validation
npm run check         # Unified verification: typecheck + all test suites
npm run gate:worker   # Worker gate: types -> meta -> unit -> smoke
npm run gate:release  # Release gate: worker + e2e + coverage + golden verification
npm run gate:qualify  # Qualification: release + package tarball + sandbox run
```

---

## 4. Testing Strategy

| Tier | Scope | Environment | Execution Speed | Location |
|---|---|---|---|---|
| **Unit** | Pure functions in `src/core`: parsers, formatters, domain classifiers | In-memory, zero I/O | Milliseconds | `tests/unit` |
| **Meta** | Verification that plan §8 matches test files and module exports | File inspection | Milliseconds | `tests/meta` |
| **Smoke** | Real process execution of `bin/discord-lab.mjs`: CLI flags, help, error exits | Local filesystem | Seconds | `tests/smoke` |
| **E2E** | Headless browser execution against CDP endpoints | Virtual browser | Seconds | `tests/e2e` |

---

## 5. Infrastructure & Quality Contours

All development strictly enforces:
- Invariant verification: Empty inputs, malformed lines, edge values, long symbol names.
- Behavior matrices: Explicit truth tables covering edge conditions.
- Strict provenance: Reference artifacts linked with MD5 checksums.

---

## 6. Parity Matrix

Existing scripts in `scripts/*` are mapped to modular functions in `src/*`, ensuring backwards compatibility and clean architectural separation.

---

## 7. Roadmap and Historical Milestones

- **M0 — Red Baseline:** Initial project layout with failing test stubs.
- **M1 — Pure Core:** Pure logic modules implemented with 100% green unit suites.
- **M2 — Frida Adapter:** Dynamic process inspection and symbol hooking.
- **M3 — UI Automation:** Autonomous interaction engine and visual logging.
- **M4 — Report Pipelines:** Static function analysis and symbol categorization.
- **M5 — Quality Contours:** Mutation testing and strict coverage enforcement.
- **M6 — Native Client Integration:** Native Rust architecture and GPUI integration.

---

## 8. Test Matrix: Test File ↔ Source Module ↔ Export Tokens

Historical Context: The initial module scaffolding in `src/` was generated directly from tests. Following completion of core milestones, this table serves as the canonical contract verification registry inspected by `tests/meta/plan-sync.test.mjs`.

| Test | Module | Exports / Contract |
| --- | --- | --- |
| `tests/unit/args.test.mjs` | `src/core/args.mjs` | `parseArgs`, `parseNumericArg`, `parseRegexArg` |
| `tests/unit/csv.test.mjs` | `src/core/csv.mjs` | `safeName`, `csv` |
| `tests/unit/paths.test.mjs` | `src/core/paths.mjs` | `projectRoot`, `logsDir`, `functionsDir`, `screensDir`, `screenshotName`, `relativeToRoot` |
| `tests/unit/processes.test.mjs` | `src/core/processes.mjs` | `isDiscordProcess`, `filterDiscordProcesses` |
| `tests/unit/inventory.test.mjs` | `src/core/inventory.mjs` | `inventoryCounts`, `inventoryCsvRows`, `inventoryJson` |
| `tests/unit/profile.test.mjs` | `src/core/ui/profile.mjs` | `topFunctions` |
| `tests/unit/blacklist.test.mjs` | `src/core/ui/blacklist.mjs` | `isBlacklisted` |
| `tests/unit/walk-log.test.mjs` | `src/core/ui/walk-log.mjs` | `parseVisited`, `buildWalkRecord`, `consoleIssues`, `shouldBail`, `pickNextItem` |
| `tests/unit/walk.test.mjs` | `src/core/ui/engine.mjs`, `src/core/ui/adapters.mjs` | `runWalk`, `resolveAdapter`, `clickNavItemExpression` |
| `tests/unit/cdp.test.mjs` | `src/core/cdp/client.mjs` | `resolveEndpoint`, `resolveTargetHint`, `listTargets`, `pickTarget`, `createCdpClient`, `evaluate`, `captureScreenshot`, `clickText`, `clickTextExpression`, `listElements`, `listElementsExpression`, `openPage` |
| `tests/unit/cli-run.test.mjs` | `src/cli/run.mjs` | `run`, `cdp targets`, `pids`, `inventory`, `hook`, `summarize`, `domains`, `static` |
| `tests/unit/calls.test.mjs` | `src/core/calls.mjs` | `isCallPayload`, `callLine`, `parseCallLog`, `summarizeCallLines` |
| `tests/unit/nm.test.mjs` | `src/core/report/nm.mjs` | `parseNmPosix`, `staticFunctionsCsv` |
| `tests/unit/domains.test.mjs` | `src/core/report/domains.mjs` | `classifyName`, `domainCsv`, `formatDomainSummary` |
| `tests/unit/frida.test.mjs` | `src/adapters/frida.mjs` | `loadDevice`, `loadAgentSource`, `listProcesses`, `attach`, `collectInventory`, `parseHookMessage`, `startHook`, `stopHook` |
| `tests/meta/plan-sync.test.mjs` | — | meta: §8 <-> tests <-> modules <-> exports |
| `tests/meta/fixtures-tracked.test.mjs` | — | meta: fixtures `tests/fixtures` tracked by `git` |
| `tests/smoke/cli.smoke.test.mjs` | `bin/chord.mjs`, `bin/discord-lab.mjs`, `src/cli/run.mjs` | `--help`, `--version` |
| `tests/smoke/pids.smoke.test.mjs` | — | `pids` (Frida, skip-aware) |
| `tests/smoke/inventory.smoke.test.mjs` | — | `inventory` (unreachable pid -> exit code 2) |
| `tests/smoke/hook.smoke.test.mjs` | — | `hook` (unreachable pid -> exit code 2) |
| `tests/smoke/summarize.smoke.test.mjs` | — | `summarize` (parity summarize-calls.sh) |
| `tests/smoke/cxxfilt.smoke.test.mjs` | `src/adapters/cxxfilt.mjs` | `demangle` |
| `tests/e2e/cdp-browser.e2e.test.mjs` | — | `targets`, `eval`, `clicktext`, `list`, `shot` |
| `tests/e2e/walk-fixture.e2e.test.mjs` | — | `walk` |
| `tests/e2e/live-discord.e2e.test.mjs` | — | `targets`, `eval` (read-only, opt-in) |
| `tests/unit/hook-io.test.mjs` | `src/core/hook-io.mjs` | `openCallLog`, `waitForStop` |
| `tests/unit/frida-agent.test.mjs` | — | `inventory`, `hook`, `unhook` |
| `tests/unit/gui-spike-bench.test.mjs` | `scripts/gui-spike-bench.mjs` | `summarize`, `percentile95`, `parseStatus`, `parseCliArgs` |
| `tests/unit/test-impact.test.mjs` | `scripts/test-impact.mjs` | `planFor`, `docs-sync`, `--full` |
| `tests/unit/gui-spike-model.test.mjs` | `rust/gui-spike/ui/model.js` | `PageStore`, `getRow`, `ensureRange`, `stats` |
| `tests/unit/channel-paging.test.mjs` | `rust/gui-spike/src/data.rs` | `messages_page`, `nextCursor`, `hasMore`, `loaded_rows_limit` |
| `tests/unit/gui-spike-dom.test.mjs` | `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/app.js` | DOM contract: 4 screens, mandatory ids, constants |
| `tests/unit/gui-spike-features.test.mjs` | `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/features/settings.js`, `rust/gui-spike/ui/features/emoji.js`, `rust/gui-spike/ui/features/search-overlay.js` | `loadApp`, `data-action`, `settings`, `emoji`, `search` |
| `tests/unit/dm-friends-ui.test.mjs` | `rust/gui-spike/ui/features/dm-friends.js` | `createDmFriendsController`, `DM_FRIENDS_ENDPOINTS`, `FRIENDS_TABS`, `applyDmFriendsDispatch`, `sortPrivateChannels` |
| `tests/meta/test-policy.test.mjs` | — | `TEST-POLICY`, `--no-verify`, `gate.mjs` |
| `tests/meta/design-breakpoints.test.mjs` | — | meta: responsive shell 1600/1200/960/720 (`--layout-tree-w`, `--layout-tree-min-w`, `#member-panel`, `channel-tree`) |
| `tests/meta/design-tokens-contrast.test.mjs` | — | meta: Design Tokens V2 and WCAG 2.2 contrast (`WCAG 2.2`, `design-tokens-v2.md`, `DESIGN-TOKENS-V2.md`, `contrastRatio`, `relativeLuminance`, `Midnight`) |
| `tests/unit/gui-spike-regressions.test.mjs` | — | `quick-switcher`, `quick-switcher-empty`, `msg-toolbar`, `delete_message` |
| `tests/unit/channel-switch.test.mjs` | — | `channels`, `messages_page`, `channel-item`, `chat-title-text`, `G6-D-01` |
| `tests/unit/guild-rail.test.mjs` | — | `ready`, `guild-rail`, `data-guild-id` |
| `tests/unit/voice-ui.test.mjs` | — | `voice-panel`, `voice-spec`, `data-state` |
| `tests/unit/voice-devices-settings.test.mjs` | — | `createVoiceDevicesSettings`, `DeviceFallback`, `NoDeviceAvailable` |
| `tests/unit/attachments-render.test.mjs` | — | `renderChannelMessage`, `decodeWaveform`, `IS_VOICE_MESSAGE` |
| `tests/unit/notifications-ui.test.mjs` | — | `MUTE_TIMER_PRESETS`, `MESSAGE_NOTIFICATION_LEVELS`, `NOTIFY_HIGHLIGHTS` |
| `tests/unit/join-server.test.mjs` | — | `join-server`, `resolve_invite`, `accept_invite` |
| `tests/unit/create-guild.test.mjs` | `rust/gui-spike/ui/features/create-guild.js` | STUB-03, `create_guild`, `create-guild-dialog` |
| `tests/unit/create-guild-wizard.test.mjs` | `rust/gui-spike/ui/features/create-guild.js` | `CREATE_GUILD_TEMPLATES`, `validateGuildIconDataUri`, `createCreateGuildWizard` |
| `tests/unit/read-state-ui.test.mjs` | — | `read_state`, `ack_message`, `chat-unread-divider` |
| `tests/unit/chat-reactions.test.mjs` | — | `toggle_reaction`, `messageId`, `reaction` |
| `tests/unit/typing-ui.test.mjs` | — | `typing-indicator`, `send_typing`, `TYPING_RESEND_MS` |
| `tests/unit/gui-spike-auth.test.mjs` | `rust/gui-spike/ui/features/auth.js`, `rust/gui-spike/ui/app.js` | `auth-gate`, `auth_login_token`, `auth_status` |
| `tests/smoke/gui-spike.smoke.test.mjs` | — | live smoke: `window-ready`, `UI_SNAPSHOT` (`GUI_SPIKE_E2E=1`) |
| `tests/smoke/gui-window.smoke.test.mjs` | `scripts/gui-window-probe.py` | AT-SPI/X11: window, elements, PNG, GBM |
| `tests/meta/docs-sync.test.mjs` | — | meta: `HELP` <-> `README.md` (CLI commands documented) |
| `tests/meta/backlog-sync.test.mjs` | — | meta: cards `docs/feature-backlog.md` <-> README/TASKS/map |
| `tests/meta/rust-client-sync.test.mjs` | — | meta: Rust client plan (docs/rust-client) <-> README/TASKS/map |
| `tests/meta/waves-sync.test.mjs` | — | meta: `waves/db.jsonl` (schema v1) <-> `STATE.md` and `evidence` |
| `tests/meta/keyring-live.test.mjs` | — | meta: live keyring `#[ignore]` <-> `--ignored` <-> `failOnSkips` |
| `tests/meta/ipc-path.test.mjs` | — | meta: IPC path safety — `scan` signatures `#[tauri::command]` (`PathBuf`, `File`, `path`/`root`/`dir`), `tauri-plugin-fs`/`tauri-plugin-dialog`, 0 violations |
| `tests/meta/ci-browser.test.mjs` | — | meta: CI `qa-browser` — pinned Chromium, `CHROME_BIN`, preflight, `--test-concurrency=1`, `.gate` |
| `tests/meta/e2e-required.test.mjs` | — | meta: `m2-scenarios.json`, `failOnSkips`, `requireBrowser`, `InfraError` |
| `tests/meta/mutations.test.mjs` | — | `15/15`, `mutations`, `killed`, `LatestFrameSlot`, `DAVE`, `hCaptcha` |
| `tests/meta/parity-matrix.test.mjs` | — | `MERGE-READINESS-V0.5.0.md`, `RELEASE-NOTES-V0.5.0.md`, `parseParityRows`, `verifyDomainSummary` |
| `tests/meta/model-routing.test.mjs` | — | meta: `resolveModelHarmony`, `core_carbon`, `gemini-3.8-flash-low`, `gemini-4-argon-medium` |
| `tests/unit/dispatch_ui.test.mjs` | — | `gateway://dispatch`, `FORWARDED_DISPATCHES`, `createDispatchBridge`, `READY`, `PRESENCE_UPDATE` |
| `tests/unit/search-remote-real-index.test.mjs` | — | `search_remote`, `data-guild-id`, `ready`, `002-READY.json` |
| `tests/e2e/m2-login-ready-rail.e2e.test.mjs` | — | `auth_status`, `auth_login_token`, `ready`, `002-READY.json` |
| `tests/e2e/m2-channel-send.e2e.test.mjs` | — | `M2_IDS`, `m2Channels`, `MESSAGE_CREATE`, `openApp` |
| `tests/e2e/m2-search-error.e2e.test.mjs` | — | `search_remote`, `data-guild-id`, `openApp` |
| `tests/e2e/m2-invite.e2e.test.mjs` | — | `resolve_invite`, `accept_invite`, `JOIN_TEXT`, `openApp` |
| `tests/e2e/m2-settings-reload.e2e.test.mjs` | — | `openApp`, `m2FixtureData` |
| `tests/e2e/m2-reconnect.e2e.test.mjs` | — | `NET_STATUSES`, `NET_TEXT`, `net_connect`, `openApp` |
| `tests/e2e/m2-voice-idle.e2e.test.mjs` | — | `VOICE_TEXT`, `disabled`, `openApp` |
| `tests/e2e/e2e-build006.test.mjs` | — | `openApp`, `captcha-modal`, `create-guild-dialog`, `stream-viewer`, `search-popover`, `context-menu`, `advanced-settings`, `resolve_gift_code` |
| `tests/unit/composer.test.mjs` | `rust/gui-spike/ui/features/composer.js` | `ATTACH_LIMITS`, `ATTACH_TEXT`, `GIFT_TEXT`, `createComposer`, `formatBytes` |
| `tests/unit/server-menu.test.mjs` | `rust/gui-spike/ui/features/actions.js` | `MENU_LABELS`, `REASONS`, `loadApp` |
| `tests/unit/account-dynamic-sync.test.mjs` | — | — |
| `tests/unit/advanced-settings.test.mjs` | `rust/gui-spike/ui/features/advanced-settings.js` | `createAdvancedSettingsController`, `detectKeybindConflicts`, `resolveEntranceSoundForGuild`, `normalizeClipLengthSeconds` |
| `tests/unit/appearance-settings.test.mjs` | — | — |
| `tests/unit/attachment-helper.test.mjs` | — | — |
| `tests/unit/attachment-validator.test.mjs` | — | — |
| `tests/unit/attachments-ui.test.mjs` | — | — |
| `tests/unit/audit-log.test.mjs` | — | — |
| `tests/unit/automod-editor.test.mjs` | — | — |
| `tests/unit/bindings-permissions.test.mjs` | — | — |
| `tests/unit/captcha-modal.test.mjs` | `rust/gui-spike/ui/features/captcha-modal.js` | `createCaptchaModal`, `extractCaptchaChallenge`, `captcha-modal` |
| `tests/unit/call_stream_dispatch.test.mjs` | — | — |
| `tests/unit/call_stream_lifecycle.test.mjs` | — | — |
| `tests/unit/stream_restart_lifecycle.test.mjs` | — | — |
| `tests/unit/screen_color_and_window_target.test.mjs` | — | — |
| `tests/unit/stream_broadcaster_handshake_and_source.test.mjs` | — | — |
| `tests/unit/stream_viewer_close_and_source_switch.test.mjs` | — | — |
| `tests/unit/krisp-vad-stability.test.mjs` | — | — |
| `tests/unit/guild-member-update-permissions.test.mjs` | — | — |
| `tests/unit/stream-pip-minimize-channel-switch-task806.test.mjs` | — | — |
| `tests/unit/stream-regression-task779.test.mjs` | — | — |
| `tests/unit/stream-stage-viewer-overlay-task805.test.mjs` | — | — |
| `tests/unit/stream_broadcaster_watch_isolation.test.mjs` | — | — |
| `tests/unit/task-800-av1-negotiation.test.mjs` | — | — |
| `tests/unit/task-801-nvenc-hw-encoder.test.mjs` | — | — |
| `tests/unit/task-802-vp8-vp9-transport-options.test.mjs` | — | — |
| `tests/unit/task-807-poll-audio-device-levels.test.mjs` | — | — |
| `tests/unit/task-817-captcha-webview.test.mjs` | — | — |
| `tests/unit/voice-state-merge-anti-flicker.test.mjs` | — | — |
| `tests/unit/reaction-verification.test.mjs` | — | — |
| `tests/unit/rules-screening-banner-modal.test.mjs` | — | — |
| `tests/unit/message-components.test.mjs` | — | — |
| `tests/unit/click_message_component.test.mjs` | — | — |
| `tests/unit/captcha-modal-interactive.test.mjs` | — | — |
| `tests/unit/search-popover-ui.test.mjs` | — | — |
| `tests/unit/context-menu-ui.test.mjs` | — | — |
| `tests/meta/ci-xvfb.test.mjs` | — | — |
| `tests/unit/channel-gate.test.mjs` | — | — |
| `tests/unit/channel-tree.test.mjs` | — | — |
| `tests/unit/composer-autocomplete.test.mjs` | — | — |
| `tests/unit/context-menu.test.mjs` | — | — |
| `tests/unit/embed-formatter.test.mjs` | — | — |
| `tests/unit/event-banner.test.mjs` | — | — |
| `tests/unit/events-modal.test.mjs` | — | — |
| `tests/unit/expression-picker.test.mjs` | — | — |
| `tests/unit/forum-filter.test.mjs` | — | — |
| `tests/unit/forum-view.test.mjs` | — | — |
| `tests/unit/friends-home.test.mjs` | — | — |
| `tests/unit/gift-modal.test.mjs` | `rust/gui-spike/ui/features/gift-modal.js` | `createGiftModal`, `resolveGiftStyle`, `extractGiftCodesFromText`, `classifyGiftError` |
| `tests/unit/guild-folders.test.mjs` | — | — |
| `tests/unit/guild-rail-incremental.test.mjs` | — | — |
| `tests/unit/guild-modals.test.mjs` | — | — |
| `tests/unit/i18n.test.mjs` | — | — |
| `tests/unit/inbox.test.mjs` | — | — |
| `tests/unit/inbox-panel.test.mjs` | — | — |
| `tests/unit/invite-modal.test.mjs` | — | — |
| `tests/unit/invite-parser.test.mjs` | — | — |
| `tests/unit/keybinds.test.mjs` | — | — |
| `tests/unit/locale-switcher.test.mjs` | — | — |
| `tests/unit/markdown-helper.test.mjs` | — | — |
| `tests/unit/member-list.test.mjs` | — | — |
| `tests/unit/members-subscriptions.test.mjs` | — | — |
| `tests/unit/message-actions.test.mjs` | — | — |
| `tests/unit/message-context-actions.test.mjs` | — | — |
| `tests/unit/message-media-polls.test.mjs` | — | — |
| `tests/unit/message-delete-voice-exit-ui.test.mjs` | — | — |
| `tests/unit/notifications.test.mjs` | — | — |
| `tests/unit/path1-frameworks.test.mjs` | — | — |
| `tests/unit/pins-threads.test.mjs` | — | — |
| `tests/unit/plural-format.test.mjs` | — | — |
| `tests/unit/poll-card.test.mjs` | — | — |
| `tests/unit/poll-progress.test.mjs` | — | — |
| `tests/unit/presence-activity.test.mjs` | — | — |
| `tests/unit/profile-modal.test.mjs` | — | — |
| `tests/unit/qr-auth-modal.test.mjs` | `rust/gui-spike/ui/features/qr-auth-modal.js` | `createQrAuthModal`, `parseQrAuthUrl`, `evaluateQrSecurityIndicator`, `classifyQrAuthError` |
| `tests/unit/quick-switcher.test.mjs` | — | — |
| `tests/unit/reactions-ui.test.mjs` | — | — |
| `tests/unit/reactive-store.test.mjs` | — | — |
| `tests/unit/role-editor.test.mjs` | — | — |
| `tests/unit/role-permissions.test.mjs` | — | — |
| `tests/unit/search-panel.test.mjs` | — | — |
| `tests/unit/search-popover.test.mjs` | — | — |
| `tests/unit/settings-modal.test.mjs` | — | — |
| `tests/unit/shell-bridge.test.mjs` | — | — |
| `tests/unit/shortcuts.test.mjs` | — | — |
| `tests/unit/slowmode.test.mjs` | — | — |
| `tests/unit/stage-channel.test.mjs` | — | — |
| `tests/unit/stream-viewer.test.mjs` | — | — |
| `tests/unit/stream-viewer-ui.test.mjs` | — | — |
| `tests/unit/stream-signaling-parity.test.mjs` | — | — |
| `tests/unit/svg-icons.test.mjs` | — | — |
| `tests/unit/threads-drawer.test.mjs` | — | — |
| `tests/unit/token-map.test.mjs` | — | — |
| `tests/unit/tooltip.test.mjs` | — | — |
| `tests/unit/typing-indicator.test.mjs` | — | — |
| `tests/unit/unread-bar.test.mjs` | — | — |
| `tests/unit/user-popout.test.mjs` | — | — |
| `tests/unit/voice-call-phases-and-server-access.test.mjs` | — | — |
| `tests/unit/voice-grid.test.mjs` | — | — |
| `tests/unit/voice-input-mode.test.mjs` | — | — |
| `tests/unit/voice-quality.test.mjs` | — | — |
| `tests/unit/voice-stage-preview.test.mjs` | — | — |
| `tests/unit/voice-stage-ui.test.mjs` | — | — |
| `tests/unit/voice-stream-codes-2000-2018.test.mjs` | — | — |
| `tests/unit/voice-volume.test.mjs` | — | — |
| `tests/unit/w7-invite-pool-and-rtc-status-bar.test.mjs` | — | — |
| `tests/unit/afk-voice-status.test.mjs` | — | — |
| `tests/unit/beta-release-package-958.test.mjs` | — | — |
| `tests/unit/fuzz-report.test.mjs` | — | — |
| `tests/unit/headless-compositor-harness.test.mjs` | — | — |
| `tests/unit/invite-input-preserve.test.mjs` | — | — |
| `tests/unit/quality-gate-907.test.mjs` | — | — |
| `tests/unit/security-audit-sbom-956.test.mjs` | — | — |
| `tests/unit/soak-test-955.test.mjs` | — | — |
| `tests/unit/system-requirements-953.test.mjs` | — | — |
| `tests/unit/appimage-package-961.test.mjs` | — | — |
| `tests/unit/arch-pkgbuild-964.test.mjs` | — | — |
| `tests/unit/battery-saver-976.test.mjs` | — | — |
| `tests/unit/binary-delta-969.test.mjs` | — | — |
| `tests/unit/crash-reporter-970.test.mjs` | — | — |
| `tests/unit/dave-mls-forward-secrecy-979.test.mjs` | — | — |
| `tests/unit/deb-package-963.test.mjs` | — | — |
| `tests/unit/flatpak-manifest-962.test.mjs` | — | — |
| `tests/unit/font-fallback-matrix-981.test.mjs` | — | — |
| `tests/unit/github-actions-release-968.test.mjs` | — | — |
| `tests/unit/i18n-coverage-972.test.mjs` | — | — |
| `tests/unit/input-gestures-975.test.mjs` | — | — |
| `tests/unit/ipc-security-audit-980.test.mjs` | — | — |
| `tests/unit/pipewire-portal-971.test.mjs` | — | — |
| `tests/unit/rpm-package-966.test.mjs` | — | — |
| `tests/unit/snapcraft-manifest-967.test.mjs` | — | — |
| `tests/unit/sqlite-fts5-stress-973.test.mjs` | — | — |
| `tests/unit/update-checker-965.test.mjs` | — | — |
| `tests/unit/voice-cpu-bench-974.test.mjs` | — | — |
| `tests/unit/wan-network-profiles-978.test.mjs` | — | — |
| `tests/unit/wayland-fractional-scale-977.test.mjs` | — | — |
| `tests/unit/arm64-packaging-990.test.mjs` | — | — |
| `tests/unit/audit-log-viewer-1008.test.mjs` | — | — |
| `tests/unit/autostart-xdg-1015.test.mjs` | — | — |
| `tests/unit/cross-platform-packaging-1016.test.mjs` | — | — |
| `tests/unit/custom-emojis-stickers-1005.test.mjs` | — | — |
| `tests/unit/custom-themes-985.test.mjs` | — | — |
| `tests/unit/emoji-cache-1005a.test.mjs` | — | — |
| `tests/unit/emoji-picker-1005b.test.mjs` | — | — |
| `tests/unit/forum-channels-1001.test.mjs` | — | — |
| `tests/unit/forum-protocol-1001a.test.mjs` | — | — |
| `tests/unit/forum-view-1001b.test.mjs` | — | — |
| `tests/unit/guild-roles-permissions-1007.test.mjs` | — | — |
| `tests/unit/ingame-overlay-988.test.mjs` | — | — |
| `tests/unit/input-spellcheck-1006b.test.mjs` | — | — |
| `tests/unit/multi-account-switcher-987.test.mjs` | — | — |
| `tests/unit/multi-client-stress-989.test.mjs` | — | — |
| `tests/unit/poll-view-1004b.test.mjs` | — | — |
| `tests/unit/polls-core-1004a.test.mjs` | — | — |
| `tests/unit/polls-voting-1004.test.mjs` | — | — |
| `tests/unit/rtc-diagnostics-internals-1011.test.mjs` | — | — |
| `tests/unit/soundboard-sfx-1012.test.mjs` | — | — |
| `tests/unit/spatial-audio-hrtf-986.test.mjs` | — | — |
| `tests/unit/spellcheck-ffi-1006a.test.mjs` | — | — |
| `tests/unit/spellchecker-native-1006.test.mjs` | — | — |
| `tests/unit/spotify-mpris-presence-1013.test.mjs` | — | — |
| `tests/unit/sqlite-encrypted-store-984.test.mjs` | — | — |
| `tests/unit/steam-deck-gamepad-1014.test.mjs` | — | — |
| `tests/unit/thread-sidebar-1002b.test.mjs` | — | — |
| `tests/unit/threads-lifecycle-1002.test.mjs` | — | — |
| `tests/unit/threads-lifecycle-1002a.test.mjs` | — | — |
| `tests/unit/totp-sessions-1009.test.mjs` | — | — |
| `tests/unit/voice-afk-monitor-1010.test.mjs` | — | — |
| `tests/unit/voice-messages-waveform-1003.test.mjs` | — | — |
| `tests/unit/voice-player-1003b.test.mjs` | — | — |
| `tests/unit/voice-recorder-1003a.test.mjs` | — | — |
| `tests/unit/voice-rnnoise-983.test.mjs` | — | — |
| `tests/unit/webrtc-bwe-congestion-982.test.mjs` | — | — |
| `tests/unit/soundboard-mixer-1012b.test.mjs` | — | — |
| `tests/unit/soundboard-protocol-1012a.test.mjs` | — | — |
| `tests/unit/soundboard-view-1012c.test.mjs` | — | — |
| `tests/unit/spatial-audio-filter-1017a.test.mjs` | — | — |
| `tests/unit/rtc-metrics-1011a.test.mjs` | — | — |
| `tests/unit/spatial-radar-1017b.test.mjs` | — | — |
| `tests/unit/audit-log-parser-1008a.test.mjs` | `rust/crates/chord-core/src/audit_log.rs` | `AuditLogActionType` |
| `tests/meta/modular-wiring-contract.test.mjs` | — | — |
| `tests/meta/specta-types.test.mjs` | — | — |
| `tests/unit/channel-tree-controller.test.mjs` | — | — |
| `tests/unit/context-menu-router.test.mjs` | — | — |
| `tests/unit/emojis.test.mjs` | — | — |
| `tests/unit/markdown-mentions.test.mjs` | — | — |
| `tests/unit/mentions.test.mjs` | — | — |
| `tests/unit/message-rendering-and-emoji.test.mjs` | — | — |
| `tests/unit/modal-router.test.mjs` | — | — |
| `tests/unit/modals-fsm-contract.test.mjs` | — | — |
| `tests/unit/forward-modal.test.mjs` | — | — |
| `tests/unit/inbox-popover.test.mjs` | — | — |
| `tests/unit/rules-screening-modal.test.mjs` | — | — |
| `tests/unit/screen-share-dialog.test.mjs` | — | — |
| `tests/unit/signals.test.mjs` | — | — |
| `tests/unit/voice-call-orchestrator.test.mjs` | — | — |


### New Feature Implementation Workflow

```bash
npm run test:unit 2>&1 | head            # Inspect failing test
# 1) Open test -> 2) Implement minimal logic in src/core -> 3) Repeat
npm run check                            # Unified typecheck and full test pass
```

---


| `tests/meta/cross-platform-ci-contract.test.mjs` | `.github/workflows/cross-platform.yml` | — |
| `tests/meta/gpui-bootstrap-contract.test.mjs` | `scripts/bootstrap-gpui.mjs` | `bootstrapGpui` |
| `tests/unit/gate-command-portability-regression.test.mjs` | `scripts/gate.mjs` | `runCommand` |
| `tests/unit/gate-platform-portability.test.mjs` | `scripts/gate-platform.mjs` | `commandInvocation` |
| `tests/unit/invite-summary-portability-regression.test.mjs` | `rust/gui-spike/ui/features/invite-parser.js` | `formatInviteSummaryRu` |
| `tests/unit/source-path-migration.test.mjs` | `scripts/source-paths.mjs` | `canonicalRustSourcePath` |


| `tests/unit/release-source-names.test.mjs` | `scripts/release.sh` | — |


| `tests/unit/ui-fixture-cleanup-regression.test.mjs` | `tests/helpers/ui-fixture.mjs` | `cleanupUiFixture` |


| `tests/meta/fixture-e2e-selection.test.mjs` | `scripts/gate.mjs` | `fixtureE2eFiles` |
| `tests/unit/arm64-build-integrity.test.mjs` | `scripts/build-arm64.sh` | — |


| `tests/unit/slowmode-contract.test.mjs` | `rust/gui-spike/ui/features/slowmode.js` | `slowmode` |
| `tests/unit/app-shell-wiring-contract.test.mjs` | `rust/gui-spike/ui/features/app-shell-wiring.js` | `app-shell-wiring` |
| `tests/unit/channel-node-renderer-contract.test.mjs` | `rust/gui-spike/ui/features/channel-node-renderer.js` | `channel-node-renderer` |
| `tests/unit/channel-switch-permissions-contract.test.mjs` | `rust/gui-spike/ui/features/channel-switch-permissions.js` | `channel-switch-permissions` |
| `tests/unit/chat-helpers-bridge-contract.test.mjs` | `rust/gui-spike/ui/features/chat-helpers-bridge.js` | `chat-helpers-bridge` |
| `tests/unit/composer-popovers-bridge-contract.test.mjs` | `rust/gui-spike/ui/features/composer-popovers-bridge.js` | `composer-popovers-bridge` |
| `tests/unit/discord-native-bridge-contract.test.mjs` | `rust/gui-spike/ui/features/discord-native-bridge.js` | `discord-native-bridge` |
| `tests/unit/dispatch-bridge-contract.test.mjs` | `rust/gui-spike/ui/features/dispatch-bridge.js` | `dispatch-bridge` |
| `tests/unit/dispatch-model-contract.test.mjs` | `rust/gui-spike/ui/features/dispatch-model.js` | `dispatch-model` |
| `tests/unit/dispatch-view-contract.test.mjs` | `rust/gui-spike/ui/features/dispatch-view.js` | `dispatch-view` |
| `tests/unit/embeds-contract.test.mjs` | `rust/gui-spike/ui/features/embeds.js` | `embeds` |
| `tests/unit/errors-contract.test.mjs` | `rust/gui-spike/ui/features/errors.js` | `errors` |
| `tests/unit/expressions-manager-contract.test.mjs` | `rust/gui-spike/ui/features/expressions-manager.js` | `expressions-manager` |
| `tests/unit/floating-popover-contract.test.mjs` | `rust/gui-spike/ui/features/floating-popover.js` | `floating-popover` |
| `tests/unit/guild-boost-contract.test.mjs` | `rust/gui-spike/ui/features/guild-boost.js` | `guild-boost` |
| `tests/unit/ipc-bindings-contract.test.mjs` | `rust/gui-spike/ui/features/ipc-bindings.js` | `ipc-bindings` |
| `tests/unit/live-emoji-picker-contract.test.mjs` | `rust/gui-spike/ui/features/live-emoji-picker.js` | `live-emoji-picker` |
| `tests/unit/live-settings-enhancer-contract.test.mjs` | `rust/gui-spike/ui/features/live-settings-enhancer.js` | `live-settings-enhancer` |
| `tests/unit/message-interaction-controller-contract.test.mjs` | `rust/gui-spike/ui/features/message-interaction-controller.js` | `message-interaction-controller` |
| `tests/unit/message-toolbar-contract.test.mjs` | `rust/gui-spike/ui/features/message-toolbar.js` | `message-toolbar` |
| `tests/unit/moderation-panel-contract.test.mjs` | `rust/gui-spike/ui/features/moderation-panel.js` | `moderation-panel` |
| `tests/unit/onboarding-wizard-contract.test.mjs` | `rust/gui-spike/ui/features/onboarding-wizard.js` | `onboarding-wizard` |
| `tests/unit/pins-threads-wiring-contract.test.mjs` | `rust/gui-spike/ui/features/pins-threads-wiring.js` | `pins-threads-wiring` |
| `tests/unit/reactive-components-contract.test.mjs` | `rust/gui-spike/ui/features/reactive-components.js` | `reactive-components` |
| `tests/unit/rows-contract.test.mjs` | `rust/gui-spike/ui/features/rows.js` | `rows` |
| `tests/unit/rtc-quality-status-bar-contract.test.mjs` | `rust/gui-spike/ui/features/rtc-quality-status-bar.js` | `rtc-quality-status-bar` |
| `tests/unit/screens-contract.test.mjs` | `rust/gui-spike/ui/features/screens.js` | `screens` |
| `tests/unit/search-overlay-contract.test.mjs` | `rust/gui-spike/ui/features/search-overlay.js` | `search-overlay` |
| `tests/unit/shell-controllers-registry-contract.test.mjs` | `rust/gui-spike/ui/features/shell-controllers-registry.js` | `shell-controllers-registry` |
| `tests/unit/slash-command-picker-contract.test.mjs` | `rust/gui-spike/ui/features/slash-command-picker.js` | `slash-command-picker` |
| `tests/unit/soundboard-picker-contract.test.mjs` | `rust/gui-spike/ui/features/soundboard-picker.js` | `soundboard-picker` |
| `tests/unit/stage-request-queue-contract.test.mjs` | `rust/gui-spike/ui/features/stage-request-queue.js` | `stage-request-queue` |
| `tests/unit/thread-badge-contract.test.mjs` | `rust/gui-spike/ui/features/thread-badge.js` | `thread-badge` |
| `tests/unit/virtual-list-contract.test.mjs` | `rust/gui-spike/ui/features/virtual-list.js` | `virtual-list` |
| `tests/unit/virtualizer-contract.test.mjs` | `rust/gui-spike/ui/features/virtualizer.js` | `virtualizer` |
| `tests/unit/voice-connection-controller-contract.test.mjs` | `rust/gui-spike/ui/features/voice-connection-controller.js` | `voice-connection-controller` |
| `tests/unit/voice-diagnostics-contract.test.mjs` | `rust/gui-spike/ui/features/voice-diagnostics.js` | `voice-diagnostics` |
| `tests/unit/voice-dm-call-stage-contract.test.mjs` | `rust/gui-spike/ui/features/voice-dm-call-stage.js` | `voice-dm-call-stage` |
| `tests/unit/voice-invite-and-devices-contract.test.mjs` | `rust/gui-spike/ui/features/voice-invite-and-devices.js` | `voice-invite-and-devices` |
| `tests/unit/voice-participant-menu-contract.test.mjs` | `rust/gui-spike/ui/features/voice-participant-menu.js` | `voice-participant-menu` |
| `tests/unit/voice-popouts-controller-contract.test.mjs` | `rust/gui-spike/ui/features/voice-popouts-controller.js` | `voice-popouts-controller` |
| `tests/unit/voice-sound-effects-contract.test.mjs` | `rust/gui-spike/ui/features/voice-sound-effects.js` | `voice-sound-effects` |
| `tests/unit/voice-stage-renderer-contract.test.mjs` | `rust/gui-spike/ui/features/voice-stage-renderer.js` | `voice-stage-renderer` |
| `tests/unit/voice-stage-viewer-controls-contract.test.mjs` | `rust/gui-spike/ui/features/voice-stage-viewer-controls.js` | `voice-stage-viewer-controls` |
| `tests/unit/voice-stage-viewer-contract.test.mjs` | `rust/gui-spike/ui/features/voice-stage-viewer.js` | `voice-stage-viewer` |
| `tests/unit/voice-state-sync-contract.test.mjs` | `rust/gui-spike/ui/features/voice-state-sync.js` | `voice-state-sync` |
| `tests/unit/voice-stream-controller-contract.test.mjs` | `rust/gui-spike/ui/features/voice-stream-controller.js` | `voice-stream-controller` |
| `tests/unit/voice-stream-menus-contract.test.mjs` | `rust/gui-spike/ui/features/voice-stream-menus.js` | `voice-stream-menus` |
| `tests/unit/webhooks-editor-contract.test.mjs` | `rust/gui-spike/ui/features/webhooks-editor.js` | `webhooks-editor` |
| `tests/unit/typed-pins-commands-contract.test.mjs` | `rust/gui-spike/ui/bindings.js` | `createTypedCommands` |
| `tests/unit/static-fixture-server.test.mjs` | `tests/helpers/ui-fixture.mjs` | `startUiServer` |
| `tests/unit/bff-pins-threads-contract.test.mjs` | `rust/gui-spike/ui/features/pins-threads.js` | `createPinsThreadsController` |
| `tests/unit/modules-voice-rtc-panel-contract.test.mjs` | `rust/gui-spike/ui/modules/voice-rtc-panel.js` | `resolveVoiceCallPhaseView` |
| `tests/unit/modules-channel-tree-view-contract.test.mjs` | `rust/gui-spike/ui/modules/channel-tree-view.js` | `isConfirmedVoiceStateRecord` |
| `tests/unit/core-ui-adapters-contract.test.mjs` | `src/core/ui/adapters.mjs` | `resolveAdapter`, `clickNavItemExpression` |
| `tests/unit/core-ui-engine-contract.test.mjs` | `src/core/ui/engine.mjs` | `runWalk` |
| `tests/unit/chat-virtualizer-tanstack.test.mjs` | `rust/gui-spike/ui/features/virtualizer.js` | `VirtualList` |
| `tests/unit/chord-store-rusqlite-worker-lib-simplify-02.test.mjs` | `rust/crates/chord-store/src/worker.rs` | `STORE_MANIFEST` |
| `tests/unit/dave-e2ee-spike-lib-simplify-05.test.mjs` | `rust/dave-spike/src/bridge.rs` | `DaveVoiceBridge` |
| `tests/unit/emoji-reactions-contract.test.mjs` | `rust/gui-spike/ui/features/reactions.js` | `EMOJI_CATEGORIES` |
| `tests/unit/esm-libraries-integration.test.mjs` | `rust/gui-spike/ui/state/signals.js` | `signal` |
| `tests/unit/markdown-contract.test.mjs` | `rust/gui-spike/ui/features/markdown.js` | `parseMessageMarkdown` |
| `tests/unit/member-list-virtual.test.mjs` | `rust/gui-spike/ui/features/virtualizer.js` | `createMemberListController` |
| `tests/unit/runtime-config-stage-02.test.mjs` | `rust/crates/chord-core/src/runtime.rs` | `RuntimeConfig` |
| `tests/unit/runtime-manager-stage-02-1-2.test.mjs` | `rust/crates/chord-core/src/runtime.rs` | `RuntimeManager` |
| `tests/unit/media-pool-stage-02-1-3.test.mjs` | `rust/crates/chord-core/src/runtime.rs` | `RUNTIME_RS` |
| `tests/unit/secret-redaction-stage-02-3-1.test.mjs` | `rust/crates/chord-core/src/logging.rs` | `LOGGING_RS` |
| `tests/unit/shutdown-signals-secret-redaction-stage-02.test.mjs` | `rust/crates/chord-core/src/shutdown.rs` | `SHUTDOWN_RS` |
| `tests/unit/heartbeat-loop-stage-05-2-1.test.mjs` | `rust/crates/chord-core/src/heartbeat.rs` | `HEARTBEAT_RS` |
| `tests/unit/heartbeat-ack-stage-05-2-2.test.mjs` | `rust/crates/chord-core/src/heartbeat.rs` | `HEARTBEAT_RS` |
| `tests/unit/tokio-tungstenite-tls-stage-05-1-1.test.mjs` | `rust/crates/chord-core/src/ws_transport.rs` | `WS_TRANSPORT_RS` |
| `tests/unit/zlib-stream-stage-05-1-2.test.mjs` | `rust/crates/chord-codec/src/zlib.rs` | `ZLIB_RS` |
| `tests/unit/identify-opcode2-stage-05-3-1.test.mjs` | `rust/crates/chord-core/src/identify.rs` | `IDENTIFY_OP` |
| `tests/unit/keyring-crypto-1021a.test.mjs` | `rust/crates/chord-store/src/keyring_crypto.rs` | `SecretServiceProvider` |
| `tests/unit/resume-opcode6-stage-05-3-2.test.mjs` | `rust/crates/chord-core/src/resume.rs` | `RESUME_RS` |
| `tests/unit/stage10-bff6-dave-voice-ui.test.mjs` | `rust/gui-spike/ui/modules/voice-rtc-panel.js` | `resolveDaveE2eeIndicatorView` |
| `tests/unit/qa-socrati-qdrant-automation.test.mjs` | `scripts/qa-auto-suite.mjs` | `getEmbedding` |
| `tests/unit/totp-view-1009a.test.mjs` | `rust/crates/chord-core/src/totp_view.rs` | `TotpDialog` |
| `tests/unit/sessions-manager-1009b.test.mjs` | `rust/crates/chord-core/src/sessions_manager.rs` | `SessionInfo` |
| `tests/unit/encrypted-sqlite-1021b.test.mjs` | `rust/crates/chord-store/src/encrypted_sqlite.rs` | `encrypted_sqlite` |
| `tests/unit/spotify-mpris-1013a.test.mjs` | `rust/crates/chord-platform/src/spotify_mpris.rs` | `spotify_mpris` |
| `tests/unit/audit-log-view-1008b.test.mjs` | `rust/crates/chord-core/src/audit_log_view.rs` | `AuditLogViewer` |
| `tests/unit/gamepad-driver-1014a.test.mjs` | `rust/crates/chord-platform/src/gamepad_driver.rs` | `gamepad_driver` |
| `tests/unit/spotify-widget-1013b.test.mjs` | `rust/chord-gpui/src/profile/spotify_widget.rs` | `spotify_widget` |
| `tests/unit/voice-call-stage.test.mjs` | `rust/gui-spike/ui/features/voice-call-stage.js` | `VoiceCallStage` |
| `tests/unit/smart-typecheck-lib-simplify-06.test.mjs` | `scripts/smart-typecheck.mjs` | `LIB-SIMPLIFY-06` |
| `tests/unit/distribution-packaging-r4-08.test.mjs` | `scripts/package-dist.mjs` | `package-dist` |
| `tests/unit/store-metadata-r4-09.test.mjs` | `scripts/lint-store-metadata.mjs` | `store-metadata` |
| `tests/unit/automation-zero-token-scripts.test.mjs` | `scripts/git-sync.mjs` | `git-sync` |

## 9. Risks and Limitations

- **Browser Availability in Headless CI:** E2E CDP tests automatically skip when no Chromium binary is located; CI environments install pinned Chromium packages.
- **Live Discord Diagnostics:** Live debugging requires Discord launched with `--remote-debugging-port`. All live tests execute in strictly read-only mode.
- **Frida Capabilities:** Memory inspection requires appropriate ptrace permissions (`kernel.yama.ptrace_scope=0`).

---

## 10. Open Questions & Technical Ledger

All architectural questions regarding protocol serialization, audio mixing, and GPU rendering are logged and tracked in `DECISIONS.md`.

---

## 11. Micro-Decomposition and Worker Allocation

Individual worker task assignments, worktree branch hierarchies, and DoD criteria are maintained in `TASKS.md` and orchestrated via Orca Orchestration.
