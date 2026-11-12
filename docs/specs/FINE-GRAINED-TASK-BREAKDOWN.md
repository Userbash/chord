# Granular Task Breakdown, Implementation Micro-Plans, and Coding Annotations

This document presents a step-by-step granular decomposition of all remaining functional features of the original Discord client, identified through cleanroom audits against the `docs/re/` dump and feature parity backlogs.
Each epic is decomposed into **atomic tasks**, and each task is divided into **Strict TDD micro-steps** with exact file paths, cleanroom oracle links, and mutation testing verification criteria.

---

## 1. Block A: Community & Interactive Chat

### Epic A1: Forum Channels (`GUILD_FORUM` type 15)

#### Task TASK-1001A: Network Protocol, Gateway Parser, and Forum REST API
- **Department:** `G1 Backend` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-core/src/forum.rs`
  - `tests/unit/forum-protocol-1001a.test.mjs`
- **Oracle:** `docs/re/gateway-schema.json` (event `CHANNEL_CREATE`, flag `type: 15`), `docs/re/rest-d02.json` (`GET /channels/{id}/threads/active`).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Author unit test `tests/unit/forum-protocol-1001a.test.mjs` asserting deserialization of `available_tags`, `default_reaction_emoji`, `default_sort_order`, and `default_forum_layout`.
  2. *Micro-Step 2 (Green):* Define structs `ForumTag`, `ForumLayout`, `ForumSortOrder` in `forum.rs` with `serde` serialization.
  3. *Micro-Step 3 (Green):* Implement client method `get_active_forum_threads(channel_id)` with Snowflake pagination.
  4. *Micro-Step 4 (Mutation 5/5):* Verify killing 5 mutants (invalid type, missing moderated flag in tags, inverted sorting, empty emoji_id, infinite pagination loop).

#### Task TASK-1001B: Forum Post Grid UI `ForumGridView`
- **Department:** `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/channels/forum_view.rs`
  - `tests/unit/forum-view-1001b.test.mjs`
- **Oracle:** `docs/re/frontend-d03.json` (post card layout and tag filtering).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test verifies `ForumGridView` grid layout (card height 96px, 12px padding, responsive 2-column flow).
  2. *Micro-Step 2 (Green):* Render tag chips with multi-selection filtering and active accent highlight `#5865f2`.
  3. *Micro-Step 3 (Green):* Post card rendering: bold 15px title, author preview (20px avatar + handle), tag badges, reply and reaction tallies.
  4. *Micro-Step 4 (A11y & Mutation):* Keyboard arrow key navigation across cards and 5 killed mutants.

---

### Epic A2: Nested Discussion Threads (Threads type 11/12)

#### Task TASK-1002A: Thread Lifecycle and Auto-Archive
- **Department:** `G1 Backend` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-core/src/threads.rs`
  - `tests/unit/threads-lifecycle-1002a.test.mjs`
- **Oracle:** `docs/re/rest-d02.json` (`POST /channels/{id}/messages/{msg_id}/threads`, `PATCH /channels/{id}`).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test thread creation from message with `name`, `auto_archive_duration` (60, 1440, 4320, 10080 minutes).
  2. *Micro-Step 2 (Green):* Implement `create_thread_from_message()` and parser for `thread_metadata` (`archived`, `locked`).
  3. *Micro-Step 3 (Green):* Synchronize thread member roster on `THREAD_MEMBERS_UPDATE`.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (missing locked flag on archive, u32 timer overflow, duplicated member_id).

#### Task TASK-1002B: Thread Sidebar `ThreadSidebar`
- **Department:** `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/chat/thread_sidebar.rs`
  - `tests/unit/thread-sidebar-1002b.test.mjs`
- **Oracle:** `docs/re/frontend-d03.json` (360px chat split panel).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Validate split-view layout: fixed 360px right panel, close button, thread header.
  2. *Micro-Step 2 (Green):* Independent `virtual_list` instance for thread messages (isolated scroll buffer).
  3. *Micro-Step 3 (Green):* Main chat indicator banner below parent message: "N replies • Last 5m ago".
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (width collapse on window resize, input focus drop on thread toggle).

---

### Epic A3: Voice Messages & Waveform

#### Task TASK-1003A: Audio Recorder and Waveform Computation
- **Department:** `G5 Voice / Platform` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-voice/src/voice_message_recorder.rs`
  - `tests/unit/voice-recorder-1003a.test.mjs`
- **Oracle:** `docs/re/rest-d02.json` (`flags: 8192`, `audio/ogg; codecs=opus`, parameter `waveform`).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test encoding 48 kHz mono stream into OGG Opus (32 kbps, VBR) and generating 256-byte peak array.
  2. *Micro-Step 2 (Green):* Implement capture buffer via `cpal` with RMS calculation across 20 ms frames.
  3. *Micro-Step 3 (Green):* Normalize amplitude values to `0..255` and export as base64 string for REST.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (incorrect sample_rate, u8 overflow on normalization, duration desync).

#### Task TASK-1003B: Interactive Player `VoiceMessagePlayer`
- **Department:** `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/chat/voice_player.rs`
  - `tests/unit/voice-player-1003b.test.mjs`
- **Oracle:** `docs/re/ui7/` (voice message player component).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Render 64 or 128 vertical waveform bars (2px width, 1px radius, 1px gap).
  2. *Micro-Step 2 (Green):* Playback progress coloring played section in accent `#23a55a`.
  3. *Micro-Step 3 (Green):* Interactive scrubbing: clicking anywhere seeks playback position instantly.
  4. *Micro-Step 4 (Speed Toggle & Mutation):* Playback speed toggle button (`1x -> 1.5x -> 2x -> 1x`) and 5 killed mutants.

---

### Epic A4: In-Chat Interactive Polls (Polls & Voting)

#### Task TASK-1004A: Poll Data Model and REST Voting
- **Department:** `G1 Backend` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-core/src/polls.rs`
  - `tests/unit/polls-core-1004a.test.mjs`
- **Oracle:** `docs/re/rest-d02.json` (`PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}`, `DELETE ...`).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Deserialize `poll` payload: `question`, `answers[]`, `expiry`, `allow_multiselect`, `results`.
  2. *Micro-Step 2 (Green):* Implement `cast_poll_vote()` and `remove_poll_vote()` validating poll expiry.
  3. *Micro-Step 3 (Green):* Compute percentage distribution with rounding: $\frac{\text{votes}_i}{\sum \text{votes}} \times 100\%$.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (division by zero on 0 votes, vote acceptance on expired poll, multiselect bypass).

#### Task TASK-1004B: Poll UI Card `PollCardView`
- **Department:** `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/chat/poll_view.rs`
  - `tests/unit/poll-view-1004b.test.mjs`
- **Oracle:** `docs/re/frontend-d03.json` (poll card block in messages).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test verifies card border, checkboxes (multiselect) or radio buttons.
  2. *Micro-Step 2 (Green):* Animated progress bar fill using semi-transparent `#5865f233`.
  3. *Micro-Step 3 (Green):* Winning option badge (crown/checkmark icon) post-closure.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (missing click debounce, progress bar width exceeding 100%).

---

### Epic A5: Guild Custom Emoji and Animated Stickers

#### Task TASK-1005A: Emoji Parser and Disk LRU Cache
- **Department:** `G1 Backend` | **Priority:** `P2`
- **Files:**
  - `rust/crates/chord-media/src/emoji_cache.rs`
  - `tests/unit/emoji-cache-1005a.test.mjs`
- **Oracle:** `docs/re/frontend-d03.json` (emoji regex and CDN URLs).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Parse `<:name:snowflake>` (static) and `<a:name:snowflake>` (animated GIF).
  2. *Micro-Step 2 (Green):* Build CDN URL `https://cdn.discordapp.com/emojis/{id}.webp?size=48&quality=lossless`.
  3. *Micro-Step 3 (Green):* Two-tier cache: memory (256 MB LRU) + disk `~/.cache/discord-gpui/emojis/` validating ETag.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (path traversal via filename, cache memory leak, corrupted snowflake ID).

#### Task TASK-1005B: Emoji & Sticker Picker `EmojiPickerModal`
- **Department:** `G2 Frontend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/chat/emoji_picker.rs`
  - `tests/unit/emoji-picker-1005b.test.mjs`
- **Oracle:** `docs/re/ui8/` (emoji and sticker picker popover).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Modal 380×420px, search input, category tabs (Frequently Used, Servers, Standard).
  2. *Micro-Step 2 (Green):* Virtualized 8-column grid with smooth scroll and lazy loading.
  3. *Micro-Step 3 (Green):* Sticker tab with Lottie JSON vector frame rendering support.
  4. *Micro-Step 4 (Keyboard Nav & Mutation):* Arrow key navigation, `Escape` dismissal, 5 killed mutants.

---

### Epic A6: Native Composer Spellchecker

#### Task TASK-1006A: FFI Bindings for `libenchant-2` / `hunspell`
- **Department:** `G1 Backend` | **Priority:** `P2`
- **Files:**
  - `rust/crates/chord-platform/src/spellcheck.rs`
  - `tests/unit/spellcheck-ffi-1006a.test.mjs`
- **Oracle:** System dictionaries `/usr/share/hunspell/*.dic`.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test verifies dictionary initialization for active locale (`en_US`, `de_DE`, etc.).
  2. *Micro-Step 2 (Green):* Method `check_word(word) -> bool` filtering punctuation, URLs, emoji, and `@mentions`.
  3. *Micro-Step 3 (Green):* Method `suggest(word) -> Vec<String>` returning up to 5 best suggestions.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (skipping hyphenated words, C-string memory leaks, crash on locale switch).

#### Task TASK-1006B: Squiggly Underline Rendering and Context Menu in `MessageBar`
- **Department:** `G2 Frontend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/chat/input_spellcheck.rs`
  - `tests/unit/input-spellcheck-1006b.test.mjs`
- **Oracle:** Discord text box rendering standard.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test validates coordinate calculation for misspelled substrings debounced at 150 ms.
  2. *Micro-Step 2 (Green):* Render red squiggly line `#f23f43` under misspelled words in `gpui::TextView`.
  3. *Micro-Step 3 (Green):* Native context menu on right click displaying suggestions and "Add to Dictionary".
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (underline offset during scroll, replacing wrong word in buffer).

---

## 2. Block B: Voice & Audio Extensions

### Epic B1: Guild Soundboard

#### Task TASK-1012A: Soundboard Protocol and SFX Caching
- **Department:** `G5 Voice / Platform` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-voice/src/soundboard_protocol.rs`
  - `tests/unit/soundboard-protocol-1012a.test.mjs`
- **Oracle:** `docs/re/voice-deep.md` (Voice Gateway Opcode 18 `SOUNDBOARD_SOUND`).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test Opcode 18 payload construction: `sound_id`, `volume` (0.0..1.0), `source_guild_id`.
  2. *Micro-Step 2 (Green):* Parse inbound Voice Gateway Opcode 19 (`SPEAKING` with soundboard flag).
  3. *Micro-Step 3 (Green):* Preload and cache CDN samples in Opus 48 kHz stereo.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (volume outside 0.0..1.0, source SSRC parse failure, sample buffer leak).

#### Task TASK-1012B: Soundboard SFX Mixer into Outbound RTP
- **Department:** `G5 Voice / Platform` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-voice/src/soundboard_mixer.rs`
  - `tests/unit/soundboard-mixer-1012b.test.mjs`
- **Oracle:** RFC 3550 and Opus RTP packetization standard.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test linear summation of microphone audio and SFX with soft-clipping via `tanh()`.
  2. *Micro-Step 2 (Green):* Preserve speech intelligibility without clipping artifacts during playback.
  3. *Micro-Step 3 (Green):* Automatically terminate mixer thread when sample finishes.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (desync of 20 ms quantizing, stereo phase inversion, looping sample bug).

#### Task TASK-1012C: Soundboard Button Grid `SoundboardGridView`
- **Department:** `G2 Frontend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/voice/soundboard_view.rs`
  - `tests/unit/soundboard-view-1012c.test.mjs`
- **Oracle:** `docs/re/voice/ui7/` (soundboard selection view).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Modal view: button grid with sound emoji, label, and hotkey indicator.
  2. *Micro-Step 2 (Green):* Press animation and playback activity indicator ring.
  3. *Micro-Step 3 (Green):* Global soundboard volume slider in audio settings.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (sound spamming without debounce, overlapping hotkeys, volume reset).

---

### Epic B2: 3D Spatial Audio & HRTF Positioning

#### Task TASK-1017A: Binaural HRTF Spatial Audio Filter
- **Department:** `G5 Voice / Platform` | **Priority:** `P2`
- **Files:**
  - `rust/crates/chord-voice/src/spatial_audio.rs`
  - `tests/unit/spatial-audio-filter-1017a.test.mjs`
- **Oracle:** ISO Head-Related Transfer Function (HRTF) KEMAR dataset.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Compute interaural time difference (ITD up to 0.7 ms) and level difference (ILD up to 20 dB) by azimuth $(-180^\circ .. +180^\circ)$.
  2. *Micro-Step 2 (Green):* Fast convolution (FFT Convolver) converting participant mono stream into binaural stereo.
  3. *Micro-Step 3 (Green):* Distance attenuation $1/r$.
  4. *Micro-Step 4 (Perf & Mutation):* CPU usage $< 1.5\%$ across 10 participants, 5 killed mutants.

#### Task TASK-1017B: Participant Spatial Radar UI
- **Department:** `G2 Frontend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/voice/spatial_radar.rs`
  - `tests/unit/spatial-radar-1017b.test.mjs`
- **Oracle:** Discord spatial audio layout radar.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Circular $360^\circ$ radar with listener avatar at center and participant avatars on circumference.
  2. *Micro-Step 2 (Green):* Interactive mouse drag-and-drop to position speakers in virtual space.
  3. *Micro-Step 3 (Green):* Persist participant positioning in SQLite database.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (avatar dragging outside perimeter, coordinates lost on reconnect).

---

### Epic B3: Advanced WebRTC Diagnostics and RTC Internals (`RE-GAP-22`)

#### Task TASK-1011A: Low-Level RTCP Metrics Collector
- **Department:** `G5 Voice / Platform` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-voice/src/rtc_metrics.rs`
  - `tests/unit/rtc-metrics-1011a.test.mjs`
- **Oracle:** `waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test RTCP metrics aggregation: `fraction_lost`, `cumulative_lost`, `rtt_ms`, `jitter_ms`, `audio_bitrate_kbps`.
  2. *Micro-Step 2 (Green):* Ring buffer of last 120 samples (60 seconds at 500 ms intervals).
  3. *Micro-Step 3 (Green):* Monitor DAVE MLS: active epoch number, group size, SAS verification status.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (RTT moving average calculation error, buffer overflow, DAVE key leakage).

#### Task TASK-1011B: 60 FPS Graph Overlay `RtcDiagnosticsHud`
- **Department:** `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/voice/rtc_hud.rs`
  - `tests/unit/rtc-hud-1011b.test.mjs`
- **Oracle:** WebRTC Internals screen in official Discord.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* HUD window 640×480px, background `#1e1f22`, 3 charts (RTT, Jitter, Bitrate).
  2. *Micro-Step 2 (Green):* Vector chart rendering on WGPU at 60 FPS without dropping frame budget of main window.
  3. *Micro-Step 3 (Green):* Hover tooltips showing millisecond precision data.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (Y-axis scaling on spikes, graph freeze on transmission pause).

---

### Epic B4: Auto-Move to AFK Channel on Idle

#### Task TASK-1010A: System Idle Detector (D-Bus / Wayland)
- **Department:** `G1 Backend` | **Priority:** `P2`
- **Files:**
  - `rust/crates/chord-platform/src/idle_monitor.rs`
  - `tests/unit/idle-monitor-1010a.test.mjs`
- **Oracle:** D-Bus `org.freedesktop.ScreenSaver` and Wayland `ext-idle-notifier-v1`.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test querying idle milliseconds from native desktop interfaces.
  2. *Micro-Step 2 (Green):* Fallback between Wayland (`ext-idle-notifier-v1`) and X11 (`XScreenSaverQueryInfo`).
  3. *Micro-Step 3 (Green):* Dispatch `IdleStateChanged(is_idle: bool)` upon exceeding threshold.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (false positive on video playback, D-Bus connection leak).

#### Task TASK-1010B: AFK Channel Auto-Move Logic
- **Department:** `G5 Voice / Platform` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/voice/afk_handler.rs`
  - `tests/unit/afk-handler-1010b.test.mjs`
- **Oracle:** `docs/re/gateway-schema.json` (Voice State Update Opcode 4).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test comparing idle duration against `guild.afk_timeout` (1–60 minutes).
  2. *Micro-Step 2 (Green):* Dispatch Gateway Opcode 4 setting `channel_id` to `guild.afk_channel_id`.
  3. *Micro-Step 3 (Green):* Forcibly set `self_mute: true` in AFK channel.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (moving to AFK when `afk_channel_id` is null, repeated opcode 4 spam).

---

## 3. Block C: Server Management & Security

### Epic C1: Role Hierarchy & 64-Bit Permissions

#### Task TASK-1007A: Role Model and 64-Bit Permission Bitmask Validator
- **Department:** `G1 Backend` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-core/src/roles.rs`
  - `tests/unit/roles-permissions-1007a.test.mjs`
- **Oracle:** `docs/re/rest-d02.json` (`GET /guilds/{id}/roles`, `PATCH /guilds/{id}/roles`).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test 64-bit permission flags: `ADMINISTRATOR` ($1 \ll 3$), `MANAGE_CHANNELS` ($1 \ll 4$), `BAN_MEMBERS` ($1 \ll 2$), etc.
  2. *Micro-Step 2 (Green):* Permission validator enforcing that a user cannot assign roles with permissions exceeding their own.
  3. *Micro-Step 3 (Green):* REST method `reorder_roles(positions)` updating hierarchy order.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (privilege escalation to admin, bitmask truncation on `u64`).

#### Task TASK-1007B: Role Hierarchy Drag-and-Drop UI
- **Department:** `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/guild/roles_hierarchy.rs`
  - `tests/unit/roles-hierarchy-1007b.test.mjs`
- **Oracle:** Discord role settings view.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Role list with color chips, member count badges, and drag handle.
  2. *Micro-Step 2 (Green):* Drag-and-drop reordering with insertion line indicator.
  3. *Micro-Step 3 (Green):* Role color picker (Discord presets + custom HEX input).
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (dragging above server owner role, position index desync).

#### Task TASK-1007C: Interactive Permission Matrix
- **Department:** `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/guild/permissions_matrix.rs`
  - `tests/unit/permissions-matrix-1007c.test.mjs`
- **Oracle:** Discord role permissions view.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* 35+ permission toggles categorized into General, Text, Voice, and Advanced.
  2. *Micro-Step 2 (Green):* Danger warning banner when enabling `ADMINISTRATOR` or `MENTION_EVERYONE`.
  3. *Micro-Step 3 (Green):* Sticky "Save Changes" bar with dirty state detection.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (saving without changes, toggle state reset on tab switch).

---

### Epic C2: Server Audit Log

#### Task TASK-1008A: Audit Log Parser and Change Decoder
- **Department:** `G1 Backend` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-core/src/audit_log.rs`
  - `tests/unit/audit-log-parser-1008a.test.mjs`
- **Oracle:** `docs/re/rest-d02.json` (`GET /guilds/{id}/audit-logs`).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Parse `AuditLogActionType` values (1..145: Guild Update, Channel Create, Member Kick/Ban, Role Update).
  2. *Micro-Step 2 (Green):* Change decoder: format `old_value` and `new_value` into human-readable strings.
  3. *Micro-Step 3 (Green):* Match actor `user_id` and target `target_id` from embedded `users[]`.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (unknown action_type, missing null fields in diff, corrupted moderator ID).

#### Task TASK-1008B: Filterable Audit Log Viewer `AuditLogViewer`
- **Department:** `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/guild/audit_log_view.rs`
  - `tests/unit/audit-log-view-1008b.test.mjs`
- **Oracle:** Server Settings Audit Log view.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Log entries: action type icon, moderator handle, event description, timestamp.
  2. *Micro-Step 2 (Green):* Dropdown filters: "Filter by Action" and "Filter by User".
  3. *Micro-Step 3 (Green):* Expandable details on click showing formatted change diffs.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (filter failure, crash on empty audit list).

---

### Epic C3: Two-Factor Authentication TOTP 2FA & Session Manager

#### Task TASK-1009A: TOTP 2FA Modal Dialog `TotpDialog`
- **Department:** `G2 Frontend` | **Priority:** `P1`
- **Files:**
  - `rust/chord-gpui/src/auth/totp_view.rs`
  - `tests/unit/totp-view-1009a.test.mjs`
- **Oracle:** Discord 2FA entry dialog.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Modal with 6 separate digit input cells styled with `#1e1f22`.
  2. *Micro-Step 2 (Green):* Auto-advance focus to next cell upon input; return on `Backspace`.
  3. *Micro-Step 3 (Green):* Clipboard paste support for full 6-digit code (`Ctrl+V`).
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (allowing non-numeric characters, submitting incomplete code < 6 digits).

#### Task TASK-1009B: Session Manager and Remote Revocation
- **Department:** `G1 Backend` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-core/src/sessions_manager.rs`
  - `tests/unit/sessions-manager-1009b.test.mjs`
- **Oracle:** `docs/re/rest-d02.json` (`GET /users/@me/auth/sessions`, `DELETE ...`).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Query sessions: ID, OS, browser, IP, location, last active timestamp.
  2. *Micro-Step 2 (Green):* Highlight current session with green "Current Device" badge.
  3. *Micro-Step 3 (Green):* "Revoke Session" action dispatching `DELETE` and removing from list.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (attempting to delete unauthorized session, losing current session badge).

---

### Epic C4: Local SQLite Storage Encryption

#### Task TASK-1021A: Encryption Key Derivation from Linux Keyring
- **Department:** `G1 Backend` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-store/src/keyring_crypto.rs`
  - `tests/unit/keyring-crypto-1021a.test.mjs`
- **Oracle:** Linux Secret Service API (`org.freedesktop.secrets`).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Test generation of 256-bit cryptographically secure master key stored in Keyring.
  2. *Micro-Step 2 (Green):* Derive database key via Argon2id with machine salt.
  3. *Micro-Step 3 (Green):* Zeroize memory buffers upon shutdown.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (plaintext key in logs, weak fallback key, memory handle leak).

#### Task TASK-1021B: Transparent SQLite Page Encryption (AES-256-GCM)
- **Department:** `G1 Backend` | **Priority:** `P1`
- **Files:**
  - `rust/crates/chord-store/src/encrypted_sqlite.rs`
  - `tests/unit/encrypted-sqlite-1021b.test.mjs`
- **Oracle:** SQLite VFS page encryption / SQLCipher specification.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Verify encrypted database on disk (zero plaintext strings in hex dump).
  2. *Micro-Step 2 (Green):* 4096-byte page encryption via AES-256-GCM with unique per-page IV.
  3. *Micro-Step 3 (Green):* WAL journal integrity and crash resistance across power loss.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (IV reuse, corrupted GCM authentication tag, WAL leak).

---

## 4. Block D: Ecosystem & Cross-Platform

### Epic D1: Spotify Integration (Listen Along & Presence)

#### Task TASK-1013A: D-Bus MPRIS Client and Gateway Presence
- **Department:** `G1 Backend` | **Priority:** `P2`
- **Files:**
  - `rust/crates/chord-platform/src/spotify_mpris.rs`
  - `tests/unit/spotify-mpris-1013a.test.mjs`
- **Oracle:** D-Bus `org.mpris.MediaPlayer2` and Gateway Opcode 3 `STATUS_UPDATE` (activity type 2).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Subscribe to `PropertiesChanged` signals from Spotify MPRIS.
  2. *Micro-Step 2 (Green):* Extract metadata: track title, artist, album, `mpris:length`, track position.
  3. *Micro-Step 3 (Green):* Dispatch activity `type: 2` (`LISTENING`) to Gateway with start/end timestamps.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (timestamp desync, frozen presence after closing Spotify).

#### Task TASK-1013B: Spotify Profile Widget and Listen Along Action
- **Department:** `G2 Frontend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/profile/spotify_widget.rs`
  - `tests/unit/spotify-widget-1013b.test.mjs`
- **Oracle:** Spotify card in Discord user profile.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Profile banner: green Spotify icon `#1db954`, album artwork, track title, scrolling marquee.
  2. *Micro-Step 2 (Green):* Real-time progress bar moving with playback.
  3. *Micro-Step 3 (Green):* "Listen Along" action sending playback command to local Spotify player.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (progress bar continuing when paused, desync exceeding 2 seconds).

---

### Epic D2: Steam Deck & Gamepad UI Mode

#### Task TASK-1014A: Gamepad Driver via `gilrs`
- **Department:** `G1 Backend` | **Priority:** `P2`
- **Files:**
  - `rust/crates/chord-platform/src/gamepad_driver.rs`
  - `tests/unit/gamepad-driver-1014a.test.mjs`
- **Oracle:** `docs/feature-backlog.md` (card `R6-01` Steam Deck Game Mode).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Initialize `gilrs` and filter axis and button events.
  2. *Micro-Step 2 (Green):* Map D-pad inputs to navigation events `NavUp/Down/Left/Right`.
  3. *Micro-Step 3 (Green):* Map L1/R1 bumpers to server navigation, L2/R2 to channel switching.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (analog stick drift without deadzone, button chatter, crash on controller disconnect).

#### Task TASK-1014B: GPUI Spatial Navigation Grid and On-Screen Keyboard
- **Department:** `G2 Frontend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/input/gamepad_focus.rs`
  - `tests/unit/gamepad-focus-1014b.test.mjs`
- **Oracle:** Steam Deck Big Picture UI standard.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Highlight active element with accent focus border `#5865f2`.
  2. *Micro-Step 2 (Green):* Automatic list scrolling as gamepad focus moves through chat and channels.
  3. *Micro-Step 3 (Green):* Trigger SteamOS on-screen keyboard via D-Bus `org.kde.kwin.VirtualKeyboard` when focusing input.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (focus lost to null, focus trap in modal).

---

### Epic D3: System Autostart (XDG Autostart & Start Minimized)

#### Task TASK-1015A: XDG Autostart Desktop Entry Generator
- **Department:** `DevOps / Integration` | **Priority:** `P2`
- **Files:**
  - `rust/crates/chord-platform/src/autostart_xdg.rs`
  - `tests/unit/autostart-xdg-1015a.test.mjs`
- **Oracle:** FreeDesktop XDG Autostart Specification.
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Create file `~/.config/autostart/discord-gpui.desktop` with mode 0644.
  2. *Micro-Step 2 (Green):* Validate entry fields: `Type=Application`, `Exec=/usr/bin/discord-gpui --start-minimized`, `X-GNOME-Autostart-enabled=true`.
  3. *Micro-Step 3 (Green):* Unlink entry file when disabled in user settings.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (incorrect binary path, desktop entry syntax corruption).

#### Task TASK-1015B: Silent Minimized Tray Startup
- **Department:** `G1 Backend` | **Priority:** `P2`
- **Files:**
  - `rust/chord-gpui/src/platform/minimized_launch.rs`
  - `tests/unit/minimized-launch-1015b.test.mjs`
- **Oracle:** `docs/feature-backlog.md` (card `R4-02` Tray and hidden start).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Inspect CLI flag `--start-minimized` in `std::env::args()`.
  2. *Micro-Step 2 (Green):* Skip `window.open()` and WGPU surface initialization until tray icon click.
  3. *Micro-Step 3 (Green):* Enforce memory ceiling when minimized: VmRSS $< 8$ MB prior to mapping window.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (accidental window flash at launch, tray freeze).

---

### Epic D4: Cross-Platform Packaging (Windows & macOS)

#### Task TASK-1016A: WiX Toolset Specification for Windows (MSI & Portable)
- **Department:** `DevOps / Integration` | **Priority:** `P2`
- **Files:**
  - `packaging/windows/wix.xml`
  - `scripts/build-windows-msi.sh`
  - `tests/unit/windows-packaging-1016a.test.mjs`
- **Oracle:** `docs/feature-backlog.md` (card `R6-03` Windows packaging).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Validate WiX XML manifest: Product GUID, binary paths, Start Menu shortcuts.
  2. *Micro-Step 2 (Green):* Configure Portable mode (profile stored under adjacent `data/` directory).
  3. *Micro-Step 3 (Green):* Windows Taskbar unread badge overlay integration.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (invalid UpgradeCode, registry modifications in portable mode).

#### Task TASK-1016B: macOS `.dmg` Packaging and Vibrancy
- **Department:** `DevOps / Integration` | **Priority:** `P2`
- **Files:**
  - `packaging/macos/build-dmg.sh`
  - `rust/chord-gpui/src/platform/macos_vibrancy.rs`
  - `tests/unit/macos-packaging-1016b.test.mjs`
- **Oracle:** `docs/feature-backlog.md` (card `R6-04` macOS packaging).
- **Implementation Micro-Plan (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Validate bundle structure `DiscordGPUI.app` (`Contents/MacOS/`, `Contents/Resources/`, `Info.plist`).
  2. *Micro-Step 2 (Green):* Compressed `.dmg` packaging script with background image and `/Applications` symlink.
  3. *Micro-Step 3 (Green):* Native background blur effect via `NSVisualEffectView` Material Dark HUD.
  4. *Micro-Step 4 (Mutation 5/5):* 5 mutants (malformed Info.plist, missing universal x86_64/arm64 binary).
