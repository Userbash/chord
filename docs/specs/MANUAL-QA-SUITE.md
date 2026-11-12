# Manual Exploratory Testing Regulations, Checklists, and Scenarios (`docs/specs/MANUAL-QA-SUITE.md`)

- **Task:** `TASK-908` (`G4 QA & Automation`)
- **Subtasks:** `SUB-908-1`, `SUB-908-2`, `SUB-908-3`
- **Related Specifications:** [`docs/specs/QA-AUTOMATION-STRATEGY-AND-TASKS.md`](docs/specs/QA-AUTOMATION-STRATEGY-AND-TASKS.md), [`TEST-POLICY.md`](TEST-POLICY.md), [`docs/specs/STREAM-WAYLAND-GUIDE.md`](docs/specs/STREAM-WAYLAND-GUIDE.md)
- **Code Modules and Contract Tests:**
  - [`rust/discord-gpui/src/manual_qa/mod.rs`](rust/discord-gpui/src/manual_qa/mod.rs)
  - [`rust/discord-gpui/src/manual_qa/checklists.rs`](rust/discord-gpui/src/manual_qa/checklists.rs)
  - [`rust/discord-gpui/src/manual_qa/desktop_matrix.rs`](rust/discord-gpui/src/manual_qa/desktop_matrix.rs)
  - [`rust/discord-gpui/src/manual_qa/exploratory.rs`](rust/discord-gpui/src/manual_qa/exploratory.rs)
  - [`rust/discord-gpui/tests/manual_qa_suite_contract.rs`](rust/discord-gpui/tests/manual_qa_suite_contract.rs)
- **Reference Clean-Room RE Oracles (`docs/re/`):**
  - `docs/re/rest-static.md` (§1 `POST /auth/login`, `/auth/mfa/totp`, `/auth/logout`, message and attachment limits)
  - `docs/re/gateway-protocol.md` and `docs/re/gateway-schema.md` (`OP 2 IDENTIFY`, `OP 4 VOICE_STATE_UPDATE`, `OP 6 RESUME`, `OP 18..22 STREAM_*`)
  - `docs/re/native-re.md` (`org.freedesktop.portal.ScreenCast`, `BaseCapturerPipeWire`, `XShmGetImage`)
  - `docs/re/voice-deep.md` (WebRTC audio devices, Push-to-Talk, DAVE MLS v1)

---

## 1. Acceptance Testing Checklists — Smoke & Sanity Checklists (`SUB-908-1`)

The acceptance test suite is mandatory before promoting any build candidate from `testing` to `beta` and `main`. The `ManualQA` component lifecycle transitions through the following states:
`checklist ready` (`ChecklistReady`) → `running bug-bash` (`RunningBugBash`) → `sign-off complete` (`SignOffComplete`).

### 1.1. Step 1.1: Authentication Checklist (`AuthChecklistValidator`)

| ID | Test Scenario | Execution Steps | Expected Result (`docs/re/rest-static.md` Oracle) | Status |
|---|---|---|---|---|
| `AUTH-01` | Username and password login (`POST /auth/login`) | 1. Open login dialog.<br>2. Enter valid email and password (`>= 8` chars).<br>3. Click "Log In". | `POST /auth/login` request yields a 2FA ticket (`LoginStep::Mfa`) or session token. Zero validation errors. | `[x]` |
| `AUTH-02` | QR Code login (Remote Auth WS) | 1. Switch to QR code panel.<br>2. Verify `discord-remote-auth-*` fingerprint generation.<br>3. Approve login on mobile device. | Crisp QR code displays without clipping; invalid fingerprint returns `InvalidQrFingerprint`. | `[x]` |
| `AUTH-03` | Captcha resolution and error code `40003` handling | 1. Simulate API error code `40003` / `captcha_key` (`hcaptcha`, `sitekey`, `rqdata`).<br>2. Complete challenge in `CaptchaModal`. | Error `40003` opens `CaptchaChallenge` modal with provided `sitekey` and `rqdata`. | `[x]` |
| `AUTH-04` | Keyring token storage and secret masking | 1. Complete successful authentication.<br>2. Verify persistence in `libsecret` (`org.freedesktop.secrets`) and application logs. | `active_token` stored; in all logs and crash dumps, token `mfa.*` is masked as `mfa.[REDACTED]`. | `[x]` |
| `AUTH-05` | Account logout (`Logout`) | 1. Open `SettingsDialog` → "My Account".<br>2. Click "Log Out". | State clears: `active_token = None`, `active_captcha = None`, resets to `LoginStep::Credentials`. | `[x]` |

### 1.2. Step 1.2: Chat Checklist (`ChatChecklistValidator`)

| ID | Test Scenario | Execution Steps | Expected Result (Boundary Invariants) | Status |
|---|---|---|---|---|
| `CHAT-01` | Text message dispatch and `2000 character` boundary | 1. Send message of length `1..=2000` chars.<br>2. Attempt sending `2001` chars and whitespace-only strings. | `2000 characters` sends immediately; `2001` chars blocked with `MessageTooLong`; whitespace-only rejected with `EmptyMessagePayload`. | `[x]` |
| `CHAT-02` | Emoji and reactions (`MessageReaction`) | 1. Open `EmojiPicker` or click "Add Reaction" on message.<br>2. Select Unicode/custom emoji (e.g. `🦀`). | Reaction counter increments by `+1`, sets `me: true` flag, re-clicking toggles state. | `[x]` |
| `CHAT-03` | Attachments up to `25 MB` and `10` files | 1. Attach file of exactly `25 MB` (`26,214,400` bytes).<br>2. Attempt attaching `25 MB + 1 byte` or `11` files. | Up to `10` files and `<= 25 MB` attach successfully; oversize payload triggers `413 Payload Too Large` (`PayloadTooLarge413`); `11` files triggers `TooManyAttachments`. | `[x]` |
| `CHAT-04` | Pinning (`Pin`), Editing (`Edit`), Deleting (`Delete`), Quoting (`Quote`) | 1. Pin message via right-click context menu.<br>2. Edit message text.<br>3. Reply with quote.<br>4. Delete original message. | Pinning sets `is_pinned = true`; editing appends `(edited)` tag; quote formats `> original text` block; deletion removes message from view. | `[x]` |

### 1.3. Step 1.3: Voice Checklist (`VoiceChecklistValidator`)

| ID | Test Scenario | Execution Steps | Expected Result (`docs/re/voice-deep.md` Oracle) | Status |
|---|---|---|---|---|
| `VOICE-01` | Connecting to voice channel | 1. Click voice channel in `ChannelSidebar`.<br>2. Verify `VoicePanel` (52px) and ping indicator. | Dispatches `OP 4 VOICE_STATE_UPDATE`, initializes `VoiceSessionState` with ping and endpoint indicator. | `[x]` |
| `VOICE-02` | Mute and Deafen toggling | 1. Click microphone button (`Mute`).<br>2. Click headphone button (`Deafen`).<br>3. Uncheck `Deafen`. | Enabling `Deafen` (`is_deafened = true`) forcibly sets `is_muted = true` (strict Discord invariant). | `[x]` |
| `VOICE-03` | Audio device switching and hot-unplug fallback | 1. Select PipeWire microphone and headphones in settings.<br>2. Physically unplug active USB mic during call. | Stream switches without crashing process; device disappearance falls back to `"Default"`. | `[x]` |
| `VOICE-04` | Push-to-Talk mode and volume level (`0..200%`) | 1. Enable `Push-to-Talk` and hold activation key.<br>2. Verify input/output volume sliders at boundaries `0%`, `100%`, `200%`, and `201%`. | Holding PTT illuminates green speaking ring `is_speaking = true`; range `0..=200%` (`0.0..=2.0`) applies normally, `> 200%` rejected with `VolumeOutOfBounds`. | `[x]` |

### 1.4. Step 1.4: Screen Sharing Checklist (`ScreenShareChecklistValidator`)

| ID | Test Scenario | Execution Steps | Expected Result (`OP 18..22` & Portal/XShm) | Status |
|---|---|---|---|---|
| `STREAM-01` | Selecting "Screens" (`Monitors`) and "Applications" (`Windows`) | 1. In active call, click "Share Your Screen".<br>2. Switch between `Monitors` and `Windows` tabs. | `ScreenShareModal` filters sources by `is_screen` with zero duplicates. | `[x]` |
| `STREAM-02` | Quality presets `720p` (`1280×720`) and `1080p` (`1920×1080`), `30`/`60` FPS | 1. Launch screen stream at `720p` @ `30 FPS`.<br>2. Verify negotiated RTP parameters (`90,000 Hz`). | Sets `is_screen_sharing = true`, resolution `1280×720`, frame rate `30 FPS`. | `[x]` |
| `STREAM-03` | Hot switching source and preset (`720p` → `1080p` 60 FPS) while streaming | 1. Without stopping stream, open source selector.<br>2. Switch from monitor `screen:0` to window `{uuid}` and preset `1080p` @ `60 FPS`. | Source and preset update on the fly (`1920×1080 @ 60 FPS`), `VoiceSessionState` continues uninterrupted. | `[x]` |

---

## 2. Linux Desktop Cross-Platform Matrix (`SUB-908-2`)

The `discord-gpui` client undergoes mandatory qualification across 4 reference Linux desktop stacks:

| Environment (`DesktopEnvironmentKind`) | Session and Environment Variables | System Tray and Notifications | Screen Capture and Scaling | Key Invariants |
|---|---|---|---|---|
| **1. GNOME 46/47 Wayland** (`Step 2.1`) | `XDG_SESSION_TYPE=wayland`<br>`XDG_CURRENT_DESKTOP=GNOME`<br>`WAYLAND_DISPLAY=wayland-0` | `AppIndicator` tray (`org.kde.StatusNotifierWatcher`) + native D-Bus notifications `org.freedesktop.Notifications` | `xdg-desktop-portal-gnome` (`org.freedesktop.portal.ScreenCast` + PipeWire) | Tray icon reflects user status (Online/DND/Unread), clicking notification focuses window. |
| **2. KDE Plasma 6 Wayland** (`Step 2.2`) | `XDG_SESSION_TYPE=wayland`<br>`XDG_CURRENT_DESKTOP=KDE`<br>`WAYLAND_DISPLAY=wayland-0` | Native StatusNotifierItem (`org.kde.StatusNotifierWatcher`) + `org.freedesktop.Notifications` | PipeWire portal + `org.kde.KWin.ScreenShot2`; **Fractional Scaling `125%` and `150%`** | At scale `125%` (`1600×900` phys.) and `150%` (`1920×1080` phys.) zero blurriness, text overlap, or layout shift of `72/240/flex-1/240` grid. |
| **3. Hyprland / Sway** (`Step 2.3`) | `XDG_SESSION_TYPE=wayland`<br>`XDG_CURRENT_DESKTOP=Hyprland:sway`<br>`WAYLAND_DISPLAY=wayland-1` | Waybar SNI (`org.kde.StatusNotifierWatcher`) + `mako`/`dunst` (`org.freedesktop.Notifications`) | `xdg-desktop-portal-hyprland` / `xdg-desktop-portal-wlr` (PipeWire) | On dynamic tile resize from `900×600` to `3840×2160`, bottom `UserPanel` (52px) is not pushed out, `min_h_0`/`min_w_0` maintain scrolling. |
| **4. X11 (`Xfce` / `i3`)** (`Step 2.4`) | `XDG_SESSION_TYPE=x11`<br>`DISPLAY=:0`<br>`WAYLAND_DISPLAY` unset | System tray XEmbed / StatusNotifier + `xfce4-notifyd` / `dunst` | Automatic fallback to **X11 MIT-SHM (`XShmGetImage`, `kFullDesktopScreenId = -1`)** | In the absence of a Wayland socket, screen capture seamlessly switches to X11 SHM without panic or black screens. |

---

## 3. Exploratory Testing Charters (`SUB-908-3`)

### 3.1. Step 3.1: Stress Input Charter (`StressInputAnalyzer`)
- **Objective:** Verify resilience of the message composer, Markdown parser, and GPUI font shaper against extreme inputs.
- **Test Corpus:**
  1. Boundary Length: message of exactly **`2000 characters`** (passes) and `2001` characters (rejected).
  2. Complex Unicode: composite emoji with Zero-Width Joiner (`👨‍👩‍👧‍👦`, `🏳️‍🌈`), Zalgo diacritics, surrogate pairs, CJK ideographs.
  3. Bidirectional **`RTL` Text**: mixing Latin/Cyrillic scripts with Arabic (`مرحبا`) and Hebrew (`שלום`), alongside control character `U+202E` (Right-to-Left Override). Enforces mandatory isolation wrappers `U+2068` (FSI) … `U+2069` (PDI) so RTL text cannot distort surrounding UI elements.
  4. Malformed Markdown: unclosed code blocks ```` ```rust ````, unmatched `**bold`, `||spoiler`, nested quotes `>>>`. Parser must balance the AST without panicking or locking the render thread.

### 3.2. Step 3.2: Rapid Server and Channel Navigation — Monkey Testing (`10 clicks per second`)
- **Objective:** Surface race conditions between async Tokio tasks and GPUI, memory leaks, and draft message loss under high-frequency navigation.
- **Load Profile:** Sequence of `>= 30` switches across servers in `ServerRail` and channels in `ChannelSidebar` at a rate of **`10 clicks per second`** (interval `100 ms`).
- **Pass Criteria:**
  - Frame rendering latency strictly **`< 8.33 ms`** (120 FPS).
  - Memory consumption `VmRSS < 60 MB` (zero leaked texture handles or event listeners).
  - Draft message entered in input field (`draft_text`) remains fully preserved.

### 3.3. Step 3.3: Network Degradation and 30-Second Disconnect Recovery
- **Objective:** Validate state machine resilience during Gateway reconnection (`Opcode 6 RESUME`).
- **Scenario Steps:**
  1. Establish active session (`READY`, capture `session_id` and `last_seq`) and type a message draft in the composer.
  2. Physically disable network interface (`ip link set eth0 down` or `nmcli networking off`) for exactly **`30 seconds`**.
  3. Re-enable network interface after `30 seconds`.
- **Pass Criteria:**
  - Client automatically completes **`Opcode 6 RESUME`** handshake (submitting `session_id` and `seq`), without prompting for user re-authentication.
  - All messages received during the 30-second window are fetched strictly in sequence with zero duplicates (`duplicate_messages_count == 0`).
  - Unsent draft in composer is preserved.

### 3.4. Step 3.4: Pre-Release Bug-Bash Protocol and Defect Severity (`P0–P3`)

Prior to every release candidate promotion (`testing -> beta`), `G4 QA` leads a synchronized 60-minute **`Bug-bash`** session joined by representatives from `G1 Backend`, `G2 Frontend`, `G5 Voice & Platform`, and `PM`.

#### Bug-bash Stages:
1. **Initialization (`checklist ready`):** Coordinator verifies binary SHA-256 hash against `SHA256SUMS` and distributes 4 Linux environments (`GNOME 46/47`, `KDE Plasma 6`, `Hyprland / Sway`, `X11`) across testers.
2. **Active Phase (`running bug-bash`):** Execution of Smoke/Sanity checklists (`SUB-908-1`) and exploratory charters (`SUB-908-3`).
3. **Standardized Defect Logging (`BugReport`):**
   Every defect ticket in the issue tracker must contain:
   - Unique ID (`BUG-908-XX`), title, and target desktop environment (`DesktopEnvironmentKind`).
   - Step-by-step reproduction instructions (`reproduction_steps`, at least 1 non-empty step).
   - Artifact path to screenshot or screen recording (`screenshot_path`).
   - Clean **`GDB`** stack trace (`gdb -batch -ex "thread apply all bt full"`), scrubbed of sensitive tokens (`mask_secret_tokens` replaces `mfa.<token>` with `mfa.[REDACTED]`).
   - Severity classification **`P0–P3`**:
     - **`P0` (Blocker):** Process panic (`panic!`/`SIGSEGV`), token exposure, auth failure, data corruption, or broken DAVE E2EE encryption. **Immediate release block.**
     - **`P1` (Critical):** Core functionality broken (message sending, voice connection, screen sharing, broken UI at `125%`/`150%` scaling). **Immediate release block.**
     - **`P2` (Minor):** Cosmetic or non-blocking UX deviations (1px tooltip misalignment, animation stutter) where a workaround exists.
     - **`P3` (Trivial):** Enhancement suggestions and minor localization polish.
4. **Sign-Off Verdict (`sign-off complete`):** Build achieves `release_approved = true` and transitions to `SignOffComplete` **strictly under the condition `P0 == 0` and `P1 == 0`**.
