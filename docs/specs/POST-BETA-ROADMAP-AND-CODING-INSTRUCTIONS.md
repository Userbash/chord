# Wave Specifications RW-47, RW-48, RW-49 and Implementation Guidelines

## 1. Universal Coding Standards and TDD Guidelines

All upcoming wave tasks strictly adhere to `AGENTS.md`, `PROCESS.md`, `TEST-POLICY.md`, and `.agents/rules/*`:

1. **Strict TDD (Red ➔ Green ➔ Refactor):**
   - Zero production code is written prior to a failing regression test (`node --test tests/unit/<test>.test.mjs` or `cargo test -p <crate> --test <contract>`).
   - Tests must assert against independent oracles (`docs/re/` dump, REA artifacts, RFCs, reference xxh64/SHA-256 hashes).
2. **Mutation Gate 5/5 (100% Mutation Kill Rate):**
   - Every unit/contract test must evaluate 5 deliberate logic mutations (inverting conditionals, bypassing permission checks, zeroing cryptographic keys, corrupting timestamps, skipping input validation).
   - The test must fail on all 5 mutations and restore the initial state (preserving and verifying MD5).
3. **Architectural Guardrails and Code Hygiene:**
   - Functions strictly `< 50 lines` of logic.
   - Source files strictly `< 800 lines` (Soft Maintainability Ceiling).
   - Strict ban on `unsafe` across all Rust modules (`#![forbid(unsafe_code)]`).
   - Scope isolation: worker write-scopes never overlap; shared infrastructure files are updated only by the coordinator.
4. **Contract Verification Against `docs/re/` Oracle:**
   - REST v9 opcodes, JSON payload schemas, permission bitmasks, and media formats are verified against clean-room oracle dumps:
     - `clean-room-oracle/gateway-schema.json`, `gateway-protocol.md` (Gateway events)
     - `clean-room-oracle/rest-d02.json`, `d02-snapshot/` (REST v9 routes)
     - `clean-room-oracle/main-arch.json`, `backend-ipc.json` (IPC, RPC)
     - `clean-room-oracle/voice-deep.md`, `voice/` (WebRTC, DAVE, RTP)

---

## 2. Wave RW-47: Community, Threads & Interactive Chat

### TASK-1001: Forum Channels `GUILD_FORUM` and Post Cards
- **Department:** `G2 Frontend` + `G1 Backend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/channels/forum_view.rs`
  - `scripts/verify-forum-channels.mjs`
  - `tests/unit/forum-channels-1001.test.mjs`
- **Implementation Guidelines:**
  1. Implement channel type `type: 15` (`GUILD_FORUM`) parser from Gateway `CHANNEL_CREATE`/`GUILD_CREATE`.
  2. Support forum tags `available_tags` (id, name, emoji_id, emoji_name, moderated).
  3. Render post card grid: title, author, creation timestamp, tags, first attachment preview, message and reaction counters.
  4. Validate post pagination via REST `GET /channels/{id}/threads/active`.

### TASK-1002: Nested Discussion Threads `PUBLIC_THREAD` and Auto-Archive
- **Department:** `G1 Backend` + `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/channels/threads.rs`
  - `tests/unit/threads-lifecycle-1002.test.mjs`
- **Implementation Guidelines:**
  1. Handle thread creation opcode from message: `POST /channels/{id}/messages/{msg_id}/threads`.
  2. Implement 360px wide thread sidebar right of the main chat with independent virtual scrolling.
  3. Support auto-archive timers (`auto_archive_duration`: 60, 1440, 4320, 10080 minutes).
  4. Contract verification for clean thread open/close states (`thread_metadata.archived: true/false`).

### TASK-1003: In-Chat Voice Messages with Waveform (OGG Opus)
- **Department:** `G5 Voice` + `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-voice/src/voice_message.rs`
  - `rust/chord-gpui/src/chat/voice_message_player.rs`
  - `tests/unit/voice-messages-waveform-1003.test.mjs`
- **Implementation Guidelines:**
  1. Capture microphone audio via `cpal` into 48 kHz mono buffer, encode to Opus 32 kbps within an OGG container.
  2. Generate 256-byte base64 peak amplitude waveform array via RMS over 20 ms frames.
  3. Render interactive waveform bars in GPUI player with click scrubbing and `1x/1.5x/2x` playback speed toggles.
  4. Dispatch via `POST /channels/{id}/messages` with `flags: 8192` (`IS_VOICE_MESSAGE`).

### TASK-1004: Interactive In-Message Polls (Polls & Voting)
- **Department:** `G1 Backend` + `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/chat/polls.rs`
  - `tests/unit/polls-voting-1004.test.mjs`
- **Implementation Guidelines:**
  1. Data model `Poll`: question (`question.text`), answer options (`answers[].poll_media`), `allow_multiselect`, `expiry`.
  2. Cast vote via `PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}`.
  3. Render vote distribution bars and voter tallies with smooth layout transitions.

### TASK-1005: Guild Custom Emoji and Animated Lottie Stickers
- **Department:** `G2 Frontend` + `G1 Backend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/chat/custom_emojis.rs`
  - `tests/unit/custom-emojis-stickers-1005.test.mjs`
- **Implementation Guidelines:**
  1. Parse custom emoji `<:name:id>` and animated `<a:name:id>` in `MessageParser`.
  2. Two-tier cache (256 MB memory LRU + disk cache `~/.cache/discord-gpui/emojis/`).
  3. Render vector Lottie stickers via embedded frame parser or cached raster frames.

### TASK-1006: Native Composer Spellchecker (hunspell / libenchant)
- **Department:** `G1 Backend` + `G2 Frontend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/chat/spellchecker.rs`
  - `tests/unit/spellchecker-native-1006.test.mjs`
- **Implementation Guidelines:**
  1. FFI bindings to system `libenchant-2.so` or `libhunspell.so` with fallback word frequency trie.
  2. Async word validation during typing debounced at 150 ms.
  3. Render red squiggly underlines below misspelled words and context menu on right click with suggestions.

---

## 3. Wave RW-48: Server Administration, Roles & Security

### TASK-1007: Guild Role Hierarchy and Permission Bitmask Editor
- **Department:** `G1 Backend` + `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/guild/roles.rs`
  - `tests/unit/guild-roles-permissions-1007.test.mjs`
- **Implementation Guidelines:**
  1. Role model: `id`, `name`, `color` (RGB u32), `hoist`, `position`, `permissions` (64-bit integer `u64`).
  2. Drag-and-drop role hierarchy reordering dispatching `PATCH /guilds/{id}/roles`.
  3. Permission matrix with 35+ toggles, enforcing that a user cannot grant permissions higher than their own highest role.

### TASK-1008: Server Audit Log (Server Audit Log & REST API v9)
- **Department:** `G1 Backend` + `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/guild/audit_log.rs`
  - `tests/unit/audit-log-viewer-1008.test.mjs`
- **Implementation Guidelines:**
  1. Query `GET /guilds/{id}/audit-logs` filtering by action type (Action Types 1..145) and actor ID.
  2. Decode `changes` (old_value, new_value) for roles, channels, permissions, bans, and webhook integrations.
  3. Formatted list view with action icons and ISO timestamps.

### TASK-1009: Two-Factor Authentication TOTP 2FA and Session Manager
- **Department:** `G1 Backend` + `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/auth/totp_dialog.rs`
  - `tests/unit/totp-sessions-1009.test.mjs`
- **Implementation Guidelines:**
  1. Modal `TotpDialog` featuring 6 isolated digit input cells with automatic focus stepping.
  2. Fetch active sessions via `GET /users/@me/auth/sessions` (client name, OS, IP, last active timestamp).
  3. Remote session revocation via `DELETE /users/@me/auth/sessions/{id}` invalidating session tokens.

### TASK-1010: Idle AFK Channel Auto-Move
- **Department:** `G1 Backend` + `G5 Voice` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/voice/afk_monitor.rs`
  - `tests/unit/voice-afk-monitor-1010.test.mjs`
- **Implementation Guidelines:**
  1. Poll system user idle duration via D-Bus `org.freedesktop.ScreenSaver` / `ext-idle-notifier-v1`.
  2. Compare against guild configuration `afk_timeout` (60, 300, 900, 1800, 3600 seconds).
  3. Upon exceeding timeout, dispatch Gateway Opcode 4 moving member to `afk_channel_id` with `self_mute: true`.

### TASK-1011: Advanced WebRTC Diagnostics and RTC Internals (`RE-GAP-22`)
- **Department:** `G5 Voice` + `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/voice/rtc_diagnostics.rs`
  - `tests/unit/rtc-diagnostics-internals-1011.test.mjs`
- **Implementation Guidelines:**
  1. Collect RTCP receiver/sender reports (packets, loss, jitter, RTT, bitrate, packetsLost).
  2. Diagnostic overlay featuring real-time latency and throughput graphs (60 FPS).
  3. Render DAVE MLS v1 end-to-end encryption status (active epoch, group ID, ratchet status).

---

## 4. Wave RW-49: Ecosystem, Soundboard & Cross-Platform

### TASK-1012: Guild Soundboard (Soundboard SFX)
- **Department:** `G5 Voice` + `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/discord-voice/src/soundboard.rs`
  - `tests/unit/soundboard-sfx-1012.test.mjs`
- **Implementation Guidelines:**
  1. Sound trigger dispatching Voice Gateway Opcode 18 (`SOUNDBOARD_SOUND`).
  2. Mix soundboard audio into local outbound RTP Opus stream with configurable volume level.
  3. Cache audio samples on disk in Opus 48 kHz stereo.

### TASK-1013: Spotify D-Bus MPRIS Integration (Listen Along)
- **Department:** `G1 Backend` + `G2 Frontend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/integrations/spotify_mpris.rs`
  - `tests/unit/spotify-mpris-presence-1013.test.mjs`
- **Implementation Guidelines:**
  1. Subscribe to D-Bus `org.mpris.MediaPlayer2.spotify` signals (`PropertiesChanged`).
  2. Extract metadata fields: `xesam:title`, `xesam:artist`, `xesam:album`, `mpris:trackid`, `mpris:length`.
  3. Publish activity type 2 (`LISTENING`) via Gateway Opcode 3 (`STATUS_UPDATE`).
  4. Implement "Listen Along" action (playback synchronization over Spotify Connect Web API).

### TASK-1014: Steam Deck & Gamepad UI Mode
- **Department:** `G2 Frontend` + `G3 Design` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/input/gamepad.rs`
  - `tests/unit/steam-deck-gamepad-1014.test.mjs`
- **Implementation Guidelines:**
  1. Poll gamepad events via `gilrs` (D-pad, analog sticks, L1/R1 bumpers, A/B/X/Y buttons).
  2. Spatial navigation grid: switch guilds via L1/R1, navigate channel tree via D-pad Up/Down, enter channel via A button.
  3. Trigger SteamOS on-screen keyboard via shortcut `Steam + X` or D-Bus `org.kde.kwin.VirtualKeyboard`.

### TASK-1015: System Autostart (XDG Autostart & Start Minimized)
- **Department:** `DevOps` + `G1 Backend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/platform/autostart.rs`
  - `tests/unit/autostart-xdg-1015.test.mjs`
- **Implementation Guidelines:**
  1. Write and remove `~/.config/autostart/discord-gpui.desktop`.
  2. CLI flag `--start-minimized`: start in headless background without mapping WGPU window, showing only tray icon.
  3. Preserve window geometry and display placement across desktop restarts.

### TASK-1016: Release Packaging for Windows (MSI/Portable) and macOS (DMG/Vibrancy)
- **Department:** `DevOps` | **Priority:** `P2`
- **Files:**
  - `packaging/windows/wix.xml`
  - `packaging/macos/build-dmg.sh`
  - `tests/unit/cross-platform-packaging-1016.test.mjs`
- **Implementation Guidelines:**
  1. WiX Toolset specification building native `.msi` installer and portable `.exe`.
  2. Packaging script building macOS `.app` bundle into `.dmg` with custom background and `/Applications` symlink.
  3. Support macOS Window Vibrancy (NSVisualEffectView Material HUD Window).
