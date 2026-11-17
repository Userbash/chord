# Community Outreach & Engineering Diary Kit

Ready-to-post, authentic technical updates for developer communities (Reddit, Hacker News, Twitter/X, and Discord groups). 

These posts focus on **concrete technical problems, engineering decisions, and solutions** rather than aggressive self-promotion.

---

## 1. Reddit `r/rust`

**Title:**  
`Show r/rust: Chord – Alternative Discord desktop client with pure-Rust DAVE MLS v1 E2EE voice`

**Body:**  
```markdown
Hey everyone!

Like many of you on Linux, I've spent years frustrated with official Discord: 1.2 GB of Electron RAM usage, CPU-heavy rendering, and broken desktop audio streaming on Wayland. When Discord began rolling out DAVE MLS v1 end-to-end voice encryption (RFC 9420), older lightweight clients like Ripcord broke.

Over the past months, we've been building **Chord** (https://github.com/Userbash/chord) — a clean-room, open-source desktop client built with a pure Rust core and Tauri 2:

**Technical Highlights:**
1. **Pure Rust DAVE MLS v1 Voice Crypto (`RFC 9420`):** We implemented the complete MLS ratchet (`chord-voice`), handling truncated 8-byte authentication tags (`Aes128GcmTruncated8`), epoch transitions, and cryptographic ratchets directly in Rust. Voice connects smoothly to modern Discord servers.
2. **Linux Screen Sharing with PipeWire & DMA-BUF:** Captures Wayland desktop/windows with zero-copy DMA-BUF via XDG Desktop Portal (`ashpd`), directly mixing system audio loopback without pulse/alsa sync drift.
3. **Memory Footprint:** Drops idle memory consumption from 1,200 MB down to ~150 MB (and <15 MB in headless mode).
4. **Reliability Architecture:** The monorepo uses a 12-tier testing pyramid with 144 UI feature suites, 315+ automated suites, and a 5/5 mutation test gate.

We're in active developer alpha (v0.1.0-alpha). The code is GPL-3.0 licensed:
GitHub: https://github.com/Userbash/chord

We'd love your feedback on our crypto and Tokio RTC architectures, and we have several "Good First Issues" open for anyone looking to hack on audio DSP, UI themes, or Linux desktop integration!
```

---

## 2. Reddit `r/linux` & `r/wayland`

**Title:**  
`Screen sharing with system audio on Wayland without Electron: how we tackled Discord's biggest Linux issue in Chord`

**Body:**  
```markdown
Hi r/linux!

Anyone who uses Discord on modern Linux distributions (Fedora, Arch, Ubuntu 24.04, NixOS) with Wayland knows the pain: screen sharing either presents a black window, drops frames, or refuses to stream application/system audio.

We set out to fix this natively in **Chord** (an open-source, Zero-Electron Discord client):

**How we solved it:**
- **Zero-Copy Video Stream:** Instead of relying on Chromium's legacy X11 DesktopCapturer, we interface directly with the XDG Desktop Portal using `ashpd`. PipeWire provides DMA-BUF file descriptors passed via Unix sockets (`SCM_RIGHTS`), letting the GPU composite frames with zero CPU copy overhead.
- **Audio Loopback Mixing:** PipeWire audio nodes are tapped directly using an internal loopback sink. We feed both the microphone stream (with optional Krisp neural noise cancellation) and desktop audio into our software mixer with soft clipping, keeping both streams perfectly synchronized.
- **Result:** Crisp 60 FPS 1080p desktop and window streaming under Wayland (KDE Plasma 6, GNOME 46/47, Sway) while using less than 180 MB of RAM total.

The project is completely free and open-source (GPL-3.0):
👉 Code & Guides: https://github.com/Userbash/chord
👉 Curated starter tasks for contributors: https://github.com/Userbash/chord/blob/tauri/docs/GOOD_FIRST_ISSUES.md

We'd love Linux enthusiasts to test screensharing across different compositors and report any edge cases!
```

---

## 3. Hacker News (`Show HN`)

**Title:**  
`Show HN: Chord – Zero-Electron Discord desktop client (Rust + Tauri 2, <180MB RAM)`

**Text:**  
```markdown
Chord is an open-source, clean-room alternative desktop client for Discord, engineered in Rust and Tauri 2 to replace Electron bloat with native speed.

Repository: https://github.com/Userbash/chord

**Why we built this:**
The official Discord client consumes 800 MB–1.6 GB of RAM, runs outdated Chromium builds on Linux, and lacks support for Wayland screen audio. Alternative web wrappers (WebCord, ArmCord) still carry the Chromium engine overhead.

**Key Technical Decisions:**
- **Tauri 2 BFF Architecture:** Uses native system WebKitGTK on Linux / WebView2 on Windows rather than bundling an entire browser. A local Rust backend-for-frontend (BFF) orchestrates the Gateway v9 WebSocket state machine and REST requests with bucket rate-limiting.
- **DAVE MLS v1 E2EE (`RFC 9420`):** Discord recently migrated voice channels to MLS-based end-to-end encryption. Chord implements the cryptographic ratchet, epoch rollout, and AES-GCM verification in pure Rust (`chord-voice`).
- **Cold Start:** Starts up in <480ms (hydrated from SQLite WAL caches).
- **Quality Gates:** 100% test pass rate across 315+ automated suites, with automated mutation fuzzer gates on every PR.

The project is currently in developer alpha preview (v0.1.0-alpha). We welcome feedback, code reviews, and contributions from the community.
```

---

## 4. Twitter / X / LinkedIn Engineering Diary Snippets

### Post A: The Memory Comparison
> Replaced 1.4 GB of Electron RAM consumption with 150 MB.
> 
> How we engineered Chord (@chord_app):
> 1. Native compiled Rust core for WebRTC, Gateway v9, and SQLite WAL caching.
> 2. Tauri 2 desktop shell using native OS webview instead of shipping Chromium.
> 3. Zero-copy ring buffers for audio capture.
> 
> Open-source on GitHub: https://github.com/Userbash/chord #RustLang #Linux #OpenSource

### Post B: The Wayland Audio Problem
> Discord on Wayland has lacked proper screen audio sharing for years.
> 
> We tackled this in Chord using PipeWire DMA-BUF file descriptors via XDG Desktop Portal + an internal audio loopback mixer in pure Rust. 
> 
> 60 FPS Wayland screen sharing with audio now works out-of-the-box on Fedora & Arch: https://github.com/Userbash/chord #Wayland #Linux #Rust
