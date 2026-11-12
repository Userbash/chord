# Chord — User Guide & Feature Reference

This guide covers end-user features, settings, keyboard shortcuts, and troubleshooting for **Chord** (`chord-gpui`), the native Linux Discord client built in Rust with GPUI and WGPU.

---

## 1. Keyboard Shortcuts & Quick Navigation

| Shortcut | Action |
|---|---|
| `Ctrl+K` | Open / close **Quick Switcher** (jump across servers, channels, and DMs) |
| `Ctrl+F` | Open / close **Message Search Drawer** (local SQLite FTS5 full-text search) |
| `Ctrl+E` | Open / close **Emoji & Sticker Picker** |
| `Ctrl+V` | Paste clipboard text into the active input field |
| `Esc` | Close the active modal, popover, or overlay |

---

## 2. Authentication & Keyring Troubleshooting

Chord supports standard token/password login, QR code authentication, and **TOTP 2FA** (`rust/chord-gpui/src/auth/totp_dialog.rs`) with active session management.

### 2.1. Secret Service Keyring (`org.freedesktop.secrets`)
Session tokens are never written to plain-text config files. Chord stores credentials in your desktop keyring via `org.freedesktop.secrets` (`rust/crates/chord-platform/`).

If login succeeds but does not persist across restarts:
1. Verify a Secret Service provider is running (`gnome-keyring-daemon` on GNOME/wlroots, `kwalletd6` on KDE Plasma 6):
   ```bash
   busctl --user list | grep -i secrets
   ```
2. Ensure your default keyring collection is unlocked upon login (PAM auto-unlock).
3. Inside Flatpak or Snap sandboxes, ensure the `org.freedesktop.secrets` portal / `password-manager-service` plug is permitted.

---

## 3. Chat, Forums, Threads, Polls & Voice Messages

- **Virtualized Chat (`chat_view.rs`):** Smooth 120 FPS scrolling across large message histories with inline Markdown, code blocks, and embeds.
- **Forum Channels (`channels/forum_view.rs`):** Supports `GUILD_FORUM` (channel type `15`) with paginated active threads, 2-column grid layout, and tag filters.
- **Nested Threads (`channels/threads.rs`, `chat/thread_sidebar.rs`):** Supports `PUBLIC_THREAD` and `PRIVATE_THREAD`, auto-archive durations (`60..10080` minutes), and member synchronization.
- **Interactive Polls (`chat/polls.rs`, `chat/poll_view.rs`):** In-chat poll rendering, live vote submission, animated score bars, and winner badges.
- **Voice Messages (`chat/voice_message_player.rs`, `chat/voice_player.rs`):** OGG Opus (`48 kHz` mono, `32 kbps`) playback and recording with 256-byte RMS waveform visualization (message flag `8192`).
- **Native Spellchecker (`chat/spellchecker.rs`, `chat/input_spellcheck.rs`):** Powered by `hunspell` / `libenchant-2` with `#f23f43` underline highlighting and context-menu suggestions.

---

## 4. Custom Emojis, Lottie Stickers & Media Cache

- **Emoji & Sticker Picker (`ui/emoji_picker.rs`, `chat/emoji_picker.rs`, `chat/custom_emojis.rs`):** Supports standard Unicode emojis, guild custom emojis, and animated Lottie stickers.
- **Two-Tier Media Cache (`rust/crates/chord-media/`):**
  - **Tier 1 (RAM):** Bounded LRU in-memory cache (`256 MB` ceiling) for instant re-rendering at 120 FPS.
  - **Tier 2 (Disk):** Local disk cache under `$XDG_CACHE_HOME/chord` (or `~/.cache/chord`).
  - To clear cached avatars, emojis, and thumbnails, remove the cache directory while Chord is closed:
    ```bash
    rm -rf "${XDG_CACHE_HOME:-$HOME/.cache}/chord/media"
    ```

---

## 5. Voice Channels, Audio Processing & DAVE MLS v1

Chord implements WebRTC voice (`48 kHz` stereo Opus, `20 ms` frames) with **DAVE MLS v1 (`RFC 9420`)** end-to-end encryption (`rust/crates/chord-voice/`).

- **Input Modes:** Choose between **Voice Activity** and **Push-to-Talk** in Audio Settings.
- **Noise Suppression:** Built-in **RNNoise** neural noise suppression filters background noise before Opus encoding.
- **360° Spatial Audio (HRTF):** Per-participant 3D positioning via the interactive **Spatial Radar** (`rust/chord-gpui/src/voice/`).
- **Soundboard SFX:** Supports Discord Voice Opcode `18` / `19` (`SPEAKING`) with a `20 ms` RTP mixer and `tanh()` soft clipping to prevent distortion.
- **RTC Diagnostics HUD:** Inspect real-time RTCP Sender/Receiver Reports, RTT, jitter, packet loss curves, and the **DAVE MLS v1 6x5 Privacy Code**.
- **AFK Auto-Move:** Uses D-Bus ScreenSaver / Wayland idle detection to automatically transfer idle users to the guild AFK channel when configured.

---

## 6. Screen Sharing (Wayland, PipeWire & X11)

- **Wayland (GNOME, KDE Plasma, Hyprland/Sway):** Uses `org.freedesktop.portal.ScreenCast` and **PipeWire DMA-BUF** zero-copy capture. See [`docs/specs/STREAM-WAYLAND-GUIDE.md`](specs/STREAM-WAYLAND-GUIDE.md) for technical details.
- **X11 Fallback:** Uses `XShmGetImage` when running in an Xorg session.
- **Hardware & System Requirements:** Check [`docs/SYSTEM-REQUIREMENTS.md`](SYSTEM-REQUIREMENTS.md) for Vulkan, PipeWire, and `llvmpipe` requirements.

---

## 7. Server Management: Roles, Permissions & Audit Log

- **Role Hierarchy & 64-bit Permissions (`guild/roles.rs`):** Drag-and-drop role reordering (`PATCH /guilds/{id}/roles`) and full 64-bit permission bitmask editor covering 41 permission flags.
- **Guild Audit Log (`guild/audit_log.rs`):** Fetches `GET /guilds/{id}/audit-logs`, decodes before/after diffs, and automatically masks sensitive fields.

---

## 8. Desktop Integrations

- **Spotify MPRIS Listen Along (`integrations/spotify_mpris.rs`):** Reads active playback via Linux MPRIS D-Bus and broadcasts Discord Gateway Opcode `3` (`LISTENING`) presence.
- **Steam Deck & Gamepad UI Mode (`input/gamepad.rs`):** Controller navigation via `gilrs` with a 4-zone focus grid and SteamOS on-screen keyboard integration via D-Bus.
- **System Tray & XDG Autostart (`platform/`):** StatusNotifierItem (`SNI`) tray integration and `~/.config/autostart/` generator supporting silent minimized startup (`VmRSS < 8 MB`).
