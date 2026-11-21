# Chord — Comprehensive User & Testing Guide

> **Document Status:** `Canonical Unified Guide (v1.0.0-beta.1 / Developer Alpha)`  
> **Target Release Package:** [`rust/dist/chord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz`](../rust/dist/chord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz)  
> **Executable Binary:** [`rust/dist/chord-gpui`](../rust/dist/chord-gpui) / `~/.local/bin/chord`  
> **Checksums:** [`rust/dist/SHA256SUMS`](../rust/dist/SHA256SUMS)  

This guide provides complete end-user documentation, feature references, beta testing guidelines, debugging instructions, and troubleshooting for **Chord** (`chord-gpui`), the lightweight native Linux Discord client written in 100% Rust with GPUI and WGPU.

---

## 1. Quick Start: Verify and Launch

### 1.1. Verify Package Checksums
Before running, verify the integrity of the downloaded package:
```bash
sha256sum -c rust/dist/SHA256SUMS
```

### 1.2. Launch in One Command
Extract and run the native binary:
```bash
# Extract the release tarball
tar -xzf rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz
cd discord-gpui-v1.0.0-beta.1/

# Run the native client
./discord-gpui
```

If installed via the developer package or symlinked to `~/.local/bin/chord`:
```bash
chord
```

### 1.3. Quick Headless Self-Diagnostics
Verify WGPU rendering pipelines, UI thread startup, and SQLite storage engines in headless mode:
```bash
./discord-gpui --headless-smoke
```

---

## 2. System Requirements & Supported Linux Distributions

Chord operates on a pure Rust graphical stack (**`gpui-kit` + WGPU**) rendering directly at 120 FPS without Chromium, Electron, or Node.js. Average RAM usage is **11–35 MB VmRSS** (under a strict `< 60 MB` memory ceiling).

### 2.1. Requirements Matrix

| Component | Minimum Requirements | Recommended (120 FPS + Go Live 60 FPS) |
| :--- | :--- | :--- |
| **CPU Architecture** | `x86_64` (`amd64`), 2 cores (SSE4.2) | `x86_64` (4+ cores, AVX2 support for software codecs) |
| **RAM** | `512 MB` free RAM (client uses `< 60 MB` VmRSS) | `4 GB+` RAM |
| **Linux Kernel & C Library** | Linux Kernel `5.15+`, **`GLIBC_2.35`** (`glibc >= 2.35`) | Linux Kernel `6.6+` / `6.11+`, `glibc 2.38+` |
| **Graphics API (GPU / Renderer)** | OpenGL 3.3 / Mesa **`llvmpipe`** (software fallback) | **Vulkan 1.2+** (Mesa RADV / Intel ANV / NVIDIA 535+) |
| **Audio Subsystem** | ALSA (`libasound.so.2`) / PulseAudio | **PipeWire 1.0+** + WirePlumber (`cpal` 48 kHz stereo) |
| **Secrets & Notifications** | D-Bus (`libdbus-1.so.3`) | `org.freedesktop.secrets` (`gnome-keyring` / `kwalletd6`) + `org.freedesktop.Notifications` |

### 2.2. Supported Distributions & Desktop Environments

- **Tested Distributions:** Ubuntu 22.04 LTS (Jammy), Ubuntu 24.04 LTS (Noble), Debian 12 (Bookworm), Fedora 39/40/41, Arch Linux.
- **Desktop Environments:**
  - **GNOME:** 42–47 (Wayland and X11 sessions).
  - **KDE Plasma:** 5.27 / 6.0–6.2 (Wayland and X11 sessions).
  - **wlroots / Tiling:** Hyprland, Sway, Wayfire.
  - **X11 Window Managers:** i3, bspwm, XFCE, AwesomeWM.

---

## 3. Keyboard Shortcuts & Quick Navigation

| Shortcut | Action |
|---|---|
| `Ctrl+K` | Open / close **Quick Switcher** (jump across servers, channels, and DMs) |
| `Ctrl+F` | Open / close **Message Search Drawer** (local SQLite FTS5 full-text search) |
| `Ctrl+E` | Open / close **Emoji & Sticker Picker** |
| `Ctrl+V` | Paste clipboard text into the active input field |
| `Esc` | Close active modal, drawer, popover, or overlay |

---

## 4. Authentication & Keyring Security

Chord supports standard email/token authentication, QR code login, and **TOTP 2FA** (`rust/chord-gpui/src/auth/totp_dialog.rs`) with active session management.

### 4.1. Secret Service Keyring (`org.freedesktop.secrets`)
Session tokens and encryption credentials are never written to plain-text configuration files. Chord securely stores them in the user's desktop keyring via `org.freedesktop.secrets` (`rust/crates/chord-platform/`).

If authentication succeeds but requires re-login after reboot:
1. Verify that a Secret Service daemon is active:
   ```bash
   busctl --user list | grep -i secrets
   ```
   *(GNOME: `gnome-keyring-daemon`, KDE: `kwalletd6` / `pam_kwallet`)*.
2. Ensure the default keyring collection is unlocked automatically on desktop login (PAM integration).
3. If running inside Flatpak or Snap, ensure the `org.freedesktop.secrets` portal permission is granted.

---

## 5. Chat, Forums, Threads, Polls & Voice Messages

- **Virtualized Chat (`chat_view.rs`):** High-efficiency virtualized list scrolling with 120 FPS fluid rendering across thousands of messages, inline Markdown formatting, syntax-highlighted code blocks, and rich embeds.
- **Forum Channels (`channels/forum_view.rs`):** Full support for `GUILD_FORUM` (channel type `15`), paginated active threads, tag filtering, and 2-column card layout.
- **Nested Threads (`channels/threads.rs`, `chat/thread_sidebar.rs`):** Supports `PUBLIC_THREAD` and `PRIVATE_THREAD`, member synchronization, and auto-archive durations (`60..10080` minutes).
- **Interactive Polls (`chat/polls.rs`, `chat/poll_view.rs`):** In-message poll rendering, real-time vote updates, animated result bars, and winner badges.
- **Voice Messages (`chat/voice_message_player.rs`):** OGG Opus (`48 kHz` mono, `32 kbps`) recording and playback with a 256-byte RMS waveform visualization canvas.
- **Native Spellchecker (`chat/spellchecker.rs`):** Integrates with system `hunspell` / `libenchant-2` dictionaries with red squiggly underlines and context-menu corrections.

---

## 6. Custom Emojis, Lottie Stickers & Media Cache

- **Emoji & Sticker Picker (`ui/emoji_picker.rs`):** Complete picker supporting Unicode emojis, custom guild emojis, skin tones, and vector animated Lottie stickers.
- **Two-Tier Media Cache Architecture (`rust/crates/chord-media/`):**
  - **Tier 1 (RAM):** Bounded LRU in-memory cache (`256 MB` soft limit) for zero-latency image decoding and display.
  - **Tier 2 (Disk):** Persistent disk cache stored under `$XDG_CACHE_HOME/chord/media` (or `~/.cache/chord/media`).
  - To safely purge cached avatars and attachments:
    ```bash
    rm -rf "${XDG_CACHE_HOME:-$HOME/.cache}/chord/media"
    ```

---

## 7. Voice Channels, Audio Processing & DAVE MLS v1

Chord implements low-latency WebRTC voice communication (`48 kHz` stereo Opus, `20 ms` frames) combined with **DAVE MLS v1 (`RFC 9420`)** end-to-end encryption (`rust/crates/chord-voice/`).

- **Input Detection:** Voice Activity Detection (VAD) with configurable thresholds and zero-latency Push-to-Talk.
- **Neural Noise Suppression:** Integrated **RNNoise** neural network engine eliminates keyboard clatter, fan noise, and room echo before encoding.
- **360° Spatial Audio (HRTF):** Real-time 3D participant positioning via the interactive **Spatial Radar** view (`rust/chord-gpui/src/voice/`).
- **Soundboard SFX:** Discord Voice Opcodes `18` / `19` (`SPEAKING`) soundboard playback with a `20 ms` RTP mixer and soft `tanh()` saturation limiting.
- **RTC Diagnostics HUD:** Live telemetry panel displaying RTCP Sender/Receiver Reports, round-trip time (RTT), jitter buffer health, packet loss graphs, and the **DAVE MLS v1 6x5 Privacy Code**.
- **Idle AFK Detection:** Automatically relocates idle users to the guild's AFK channel via D-Bus ScreenSaver and Wayland idle monitoring.

---

## 8. Screen Sharing (Wayland, PipeWire DMA-BUF & X11)

Chord features native Linux screen and window broadcasting (**Go Live**):

- **Wayland (GNOME, KDE Plasma, Hyprland, Sway):** Uses `org.freedesktop.portal.ScreenCast` and **PipeWire DMA-BUF** zero-copy frame capture for 60 FPS streaming without CPU copies.
- **File Descriptor Passing & Ptrace Safety:** Utilizes `PR_SET_PTRACER_ANY` and Unix domain socket `SCM_RIGHTS` for seamless PipeWire stream handoff between background worker daemons and the client.
- **Application Audio Sharing (Soundshare):** Captures application audio via PipeWire audio sink loopback nodes.
- **X11 Fallback:** Automatically falls back to `XShmGetImage` when running in legacy Xorg sessions.
- Detailed technical specs: [`docs/specs/STREAM-WAYLAND-GUIDE.md`](specs/STREAM-WAYLAND-GUIDE.md).

---

## 9. Server Management: Roles, Permissions & Audit Log

- **Role Hierarchy & 64-bit Permissions (`guild/roles.rs`):** Visual drag-and-drop role reordering (`PATCH /guilds/{id}/roles`) and full 64-bit permission bitmask editor covering all 41 Discord permission flags.
- **Guild Audit Log (`guild/audit_log.rs`):** Live audit log viewer decoding before/after property diffs with automatic masking of sensitive credentials and tokens.

---

## 10. Desktop & System Integrations

- **Spotify MPRIS Listen Along (`integrations/spotify_mpris.rs`):** Reads active tracks via Linux MPRIS D-Bus interfaces and updates Discord Gateway Opcode `3` (`LISTENING`) status.
- **Steam Deck & Gamepad Navigation (`input/gamepad.rs`):** Full game controller support via `gilrs`, 4-zone UI focus navigation, and SteamOS on-screen keyboard triggering.
- **System Tray & Autostart (`platform/`):** StatusNotifierItem (`SNI`) system tray icon and `~/.config/autostart/` integration supporting silent minimized launch (`VmRSS < 8 MB`).

---

## 11. Debugging, Logging & GDB Crash Diagnostics

Enable detailed diagnostic logging for troubleshooting:

```bash
# Standard informational logs
RUST_LOG=info ./discord-gpui

# Deep tracing for WebRTC, DAVE E2EE, and Gateway WebSocket
RUST_LOG=debug,chord_voice=trace,chord_core=trace ./discord-gpui
```

*Note: All authentication tokens and passwords are automatically redacted (`mfa.[REDACTED]`) in all logs.*

If a segmentation fault or crash occurs, generate a full GDB backtrace:
```bash
gdb -batch -ex "thread apply all bt full" ./chord-gpui core
```

---

## 12. Bug Reporting & Feedback

When reporting issues on GitHub, please use this template:

```markdown
### Bug Description
A clear and concise description of the issue.

### Environment
- **OS / Distribution:** (e.g. Ubuntu 24.04 LTS, Arch Linux, Fedora 41)
- **Desktop Environment & Display Server:** (e.g. GNOME 47 Wayland, KDE Plasma 6.2 Wayland, Sway)
- **GPU & Driver:** (e.g. AMD Radeon 780M Mesa 24.2, NVIDIA RTX 4070 560.35, Intel Iris Xe)
- **Audio Subsystem:** (e.g. PipeWire 1.2.5, PulseAudio 16.1)
- **Client Version:** `v1.0.0-beta.1`

### Steps to Reproduce
1. Launch Chord
2. Connect to voice channel 'General'
3. Start screen share
4. Observe issue

### Expected Behavior
Screen share streams smoothly at 60 FPS with audio.

### Actual Behavior
Video stream freezes or audio fails to loop back.

### Relevant Logs
(Attach sanitized logs from RUST_LOG=info or /tmp/chord-gui.log)

### GDB Backtrace (If Crash Occurred)
(Attach output from gdb -batch -ex "thread apply all bt full")
```

### Bug Severity Classification:
- **P0 (Critical):** Startup crash, authentication failure, audio pipeline hang, security flaw.
- **P1 (High):** Screen share failure, broken message transmission, disconnection without resume.
- **P2 (Medium):** Visual artifact, minor layout misalignment, missing localization string.
- **P3 (Low):** Minor cosmetic flaw, subtle animation glitch, non-critical telemetry delay.
