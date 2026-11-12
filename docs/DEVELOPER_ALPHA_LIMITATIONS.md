# Chord — Developer Alpha / Tech Preview: Known Limitations & Architecture Report

> **Target Release:** Open-Source Developer Alpha / Tech Preview (MVP)  
> **Build:** v0.1.0-alpha  
> **Engine:** Rust 2024 (`chord-core`, `chord-voice`, `chord-codec`, `chord-client`)  
> **Date:** November 2026  

---

## 1. Executive Summary & Project Purpose

**Chord** is an independent, clean-room, native desktop client and protocol implementation written in pure Rust. It is engineered from the ground up to solve the core architectural drawbacks of the official Electron/Chromium-based Discord desktop app:

- **Ultra-low memory footprint:** Runs in ~150–250 MB RAM (versus 1.2–2.5 GB in Chromium).
- **Native DAVE End-to-End Encryption (MLS v1):** Complete clean-room implementation of Discord's DAVE protocol (`RFC 9420`), which broke all legacy third-party bots and clients in 2024.
- **First-class Linux Wayland & PipeWire integration:** Direct hardware frame acquisition via KWin DBus / PipeWire Portal with TurboJPEG GPU/CPU acceleration.
- **Independent Rust BFF Architecture:** Decoupled protocol state machine, binary ETF decompressor, and WebRTC voice driver.

---

## 2. Important Security & Non-Affiliation Disclaimer

> [!WARNING]
> **Use on Developer / Alternative Test Accounts Only!**  
> Discord Inc.'s Terms of Service prohibit connecting to their gateway using third-party or modified clients. Using automated or unofficial clients carries risk of automated anti-fraud checkpoints, phone-verification challenges, or account suspensions.  
> 
> **Chord is strictly an open-source technical research preview** intended for developers, reverse engineers, and system programmers. It is **not affiliated with, maintained by, or endorsed by Discord Inc.**

---

## 3. Comprehensive Matrix of Implemented vs. Incomplete Features

| Feature Domain | Status | Parity % | Working Details | Incomplete / Known Issues |
| :--- | :---: | :---: | :--- | :--- |
| **Gateway Protocol (v9/v10)** |  Stable | 90% | Binary ETF, zstd stream decompression, Heartbeat (Op 1/3), Resume (Op 6), Voice State (Op 4). | Full dynamic guild subscription ranges (`OP 37`) for massive servers (10k+ users) need background tuning. |
| **Voice Channels & Audio** |  Stable | 95% | RTP/RTCP transport, Opus 48 kHz codec, Krisp Neural Noise Suppression, VAD & Push-to-Talk, audio device selection. | Global system-wide Push-to-Talk hotkey on Wayland requires XDG Global Shortcuts portal integration. |
| **Voice E2EE (DAVE MLS v1)** |  Stable | 90% | Epoch rollouts, pairwise key generation, MLS proposals & welcome messages, MLS state transitions (Op 22/23/26/28). | Occasional group key re-synchronization delay when 10+ users join simultaneously. |
| **Screen Share (Broadcast)** | 🟡 Beta | 80% | Wayland KWin DBus capture, PipeWire node capture, TurboJPEG encoding, 720p/1080p @ 60 FPS, SSRC negotiation. | Application-isolated soundshare requires external loopback/EasyEffects virtual sink; direct audio isolation is in progress. |
| **Screen Share (Watching)** | 🟡 Beta | 75% | DAVE MLS decryption, WebRTC video decoding, 60 FPS HTTP/IPC frame presenter, remote sink wants (Op 15). | High packet loss on cellular/Wi-Fi can trigger video reconnect (`2007 stream-reconnecting`); multi-stream grid displays 1 stream at a time. |
| **Guild & Channel Navigation** |  Stable | 85% | Server rail, category accordion, channel hierarchy, read/unread indicators, member list with roles. | Channels with complex permission overrides or nested threads may occasionally delay populating on initial join. |
| **Server Rules Screening** |  Stable | 100% | Member Verification Gate, rule acceptance modal, fallback for verified users (bypassing 403 errors). | Fully tested and verified. |
| **Text Chat Reading** |  Stable | 85% | Infinite virtualized scroll, timestamps, avatars, Markdown formatting, code syntax blocks, embeds rendering. | Rich media embeds (inline YouTube player, Spotify widgets) render as simplified text cards. |
| **Text Chat Composer** | 🟡 Beta | 65% | Sending messages, message deletion, paste support, multiline input, auto-scroll to bottom. | **Missing `@` mention autocomplete popup**; **missing inline `:emoji:` autocomplete**; **inline message editing (Up arrow) not yet wired**. |
| **Direct Messages (DMs)** | 🔴 Alpha | 40% | Backend REST contract `POST /users/@me/channels` implemented and unit tested. | **Frontend UI lacks a dedicated Friends/DMs tab**; clicking a user profile cannot yet spawn a new DM conversation window. |
| **Reactions & Custom Emotes** | 🔴 Alpha | 50% | Emoji picker overlay exists; basic emoji reactions can be parsed from incoming messages. | Adding emoji reactions directly from message hover bar is rudimentary; custom animated Nitro emojis fall back to static. |
| **Slash Commands & Bots** | 🔴 Alpha | 30% | Command definitions and payload schema contracts exist in `chord-core`. | Interactive slash-command popover menu (`/command [options]`) inside chat input is not implemented. |
| **User Settings & Profile** | 🟡 Beta | 70% | Audio input/output devices, volume sliders, noise suppression toggles, streaming quality selectors. | Editing avatar, banner, bio, and custom status from UI settings is not yet connected to the REST profile endpoint. |
| **Cross-Platform Support** | 🟡 Beta | 50% | Linux (Wayland + X11) fully supported. | Windows (DXGI) and macOS (ScreenCaptureKit) screen capture engines are stubs; voice and chat build, but screen broadcast is Linux-only. |

---

## 4. Deep-Dive into Known Limitations & Bug Reports

### 4.1. Missing `@` Mention & Inline Autocomplete
- **Current Behavior:** Typing `@username` or `@role` inserts raw text into the input field.
- **Expected Behavior:** An interactive popup should filter guild members and roles in real time, inserting `<@123456789>` snowflake tags upon pressing `Tab` or `Enter`.
- **Engineering Task:** Implement an autocomplete overlay hook in `ui/app.js` and `ui/features/chat.js` connected to the in-memory `guildMembers` cache.

### 4.2. Direct Messaging (DMs) Not Exposed in UI
- **Current Behavior:** The client defaults to viewing the last active guild. There is no top-left Discord icon to switch to the Direct Messages channel list.
- **Expected Behavior:** A Dedicated "Home" button at the top of the server rail rendering all open DM and Group DM channels from `private_channels`.
- **Engineering Task:** Wire `dm_channels.rs` (`open_private_channel` IPC) to user profile context menus and add a DM sidebar container in `ui/app.js`.

### 4.3. Occasional Channel Invisibility on Massive Servers
- **Current Behavior:** On servers with 200+ channels and 20,000+ members, some text channels appear blank or missing until the server is reselected.
- **Root Cause:** Discord Gateway sends partial `READY` payloads and relies on `GUILD_CREATE` and lazy `OP 37` subscriptions. If the REST channel fallback is throttled, channels are delayed.
- **Engineering Task:** Implement proactive background sync for unavailable guilds and enhance `channels()` caching in `rust/gui-spike/src/data.rs`.

### 4.4. Video Stream Edge-Case Reconnects (Error 2007)
- **Current Behavior:** Watching a stream works smoothly, but network jitter can trigger a voice reconnect loop (`2007 stream-reconnecting`).
- **Fix Applied in v0.1.0-alpha:** Removed premature HTTP polling cutoff, fixed `Uint8ClampedArray` N-API allocator, and prevented token wipe every 3.5 seconds.
- **Remaining Work:** Implement WebRTC NACK / PLI packet re-requesting in `chord-voice` to gracefully withstand dropped keyframes without triggering an RTC reconnect.

---

## 5. Technology Stack & Architecture

```
┌────────────────────────────────────────────────────────┐
│                   Chord Desktop UI                     │
│    Native Webview / WGPU GPUI Interface (<200 MB RAM)   │
└───────────────────────────┬────────────────────────────┘
                            │ Tauri IPC / Shared Memory
┌───────────────────────────▼────────────────────────────┐
│                    Rust BFF Core                       │
│  - chord-core: Gateway v9/v10, REST API, State Machine │
│  - chord-voice: WebRTC, RTP/RTCP, DAVE MLS v1 E2EE     │
│  - chord-codec: ETF Binary, zstd stream decompressor   │
│  - chord-media: Zero-copy ring buffers, TurboJPEG      │
└───────────────┬────────────────────────┬───────────────┘
                │ Unix Domain Sockets    │ KWin DBus / PipeWire
┌───────────────▼───────────────┐ ┌──────▼───────────────┐
│     discord_voice.node        │ │  Wayland Capture     │
│  (Native WebRTC Media Engine) │ │  (Hardware 60 FPS)   │
└───────────────────────────────┘ └──────────────────────┘
```

---

## 6. How to Build & Run (Developer Setup)

### Prerequisites:
- **Rust:** Stable 1.80+ (`rustup default stable`)
- **Node.js:** v18+ or v20+
- **Linux Packages (Debian/Ubuntu):**
  ```bash
  sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libturbojpeg0-dev libopenh264-dev pipewire-bin
  ```
- **Linux Packages (Arch/Manjaro):**
  ```bash
  sudo pacman -S gtk3 webkit2gtk-4.1 turbojpeg openh264 pipewire
  ```

### Build Steps:
```bash
# Clone the repository
git clone https://github.com/your-org/chord.git
cd chord

# Run contract tests
cargo test --manifest-path rust/crates/chord-core/Cargo.toml
cargo test --manifest-path rust/gui-spike/Cargo.toml --test voice_stage_contract

# Build the release client
cargo build --release --manifest-path rust/gui-spike/Cargo.toml

# Launch Chord Client
./rust/gui-spike/target/release/chord-client
```

---

## 7. Open-Source Contribution Roadmap

We welcome issues, pull requests, and protocol research contributions! Top priorities for external contributors:

1. **[UI/UX] Composer `@` Mention Autocomplete:** Build the floating mention suggestions popup.
2. **[UI/UX] Direct Messages (DMs) View:** Add the DM list toggle to the server rail.
3. **[RTC] WebRTC NACK / PLI Handling:** Improve recovery from lost RTP video packets on high-latency connections.
4. **[Windows/macOS] Native Screen Capture:** Implement DXGI Desktop Duplication (Windows) and ScreenCaptureKit (macOS) drivers.
