# discord-qt ↔ Clean-Room RE ↔ Rust Client: Analysis, Gaps, and Comparison

> Technical analysis archive: `docs/discord-qt-analysis.md`.
> Transfer backlog for missing capabilities: `docs/feature-backlog.md` (Waves R4–R6/D).

Date: 2026-10-19.  
Source: `https://github.com/ruslang02/discord-qt` (`master` branch).  
Clean-room reference artifacts: `docs/re/` (`rest-d02.md`, `gateway-protocol.md`, `voice-deep.md`).

---

## 0. Executive Summary

1. **discord-qt is an Independent TypeScript Client**: Built using NodeGUI (Qt Widgets) + `discord.js v12.5` + 16 monkey-patches (~13.4k lines across 128 TS + 32 SCSS files), 41 icons, and basic i18n locales.
2. **Project Status**: Effectively dormant since release v0.5.1 (January 2021). Licensed under GPL-3.0 (no code copied; architectural patterns and contracts referenced).
3. **Core Solutions and Contracts**: Because `discord.js` is oriented toward bot accounts, user-specific functionality was manually implemented: `READY` user fields (`user_settings`, `user_guild_settings`, `private_channels`, `notes`, `read_state`), `MESSAGE_ACK` events and REST ack tokens, user notes (`users/@me/notes/:id`), settings persistence (`users/@me/settings`), custom statuses (Activity Type 4), guild sorting/muting, and lazy guild subscriptions (Opcode 14).
4. **Missing Capabilities in discord-qt**: Lacks reactions UI, threads/forums, custom stickers, full-text search, inline replies, group DMs, friends roster page, video/screensharing/streaming, DAVE MLS E2EE, modern AEAD voice encryption modes (only legacy `xsalsa20_poly1305`), Krisp AI noise reduction, and soundboards.
5. **Architectural Value for Our Native Rust Client**: discord-qt acts as a practical map of "what is needed to bridge bot APIs to a complete user experience", but reflects an early 2021 feature set. Our clean-room reverse-engineering corpus is significantly more complete: ETF+zstd streaming, Opcode 0–43 schemas, 88 REST v9 endpoints, voice encryption modes 0–6, 5,132 design tokens, and IPC inventories.
6. **Legal Scenarios**: Scenario A (independent messenger with familiar Discord UX) is fully supported; Scenario C (third-party client directly accessing Discord servers with user tokens) carries platform TOS risks.

---

## 1. Project Specifications

| Attribute | Specification |
|---|---|
| Repository | `ruslang02/discord-qt`: "Discord client powered by Node.JS and Qt Widgets" |
| Stack | Node.js, NodeGUI `@nodegui/nodegui ^0.26` (Qt Widgets via C++ Node addons), SCSS → Qt Style Sheets, Webpack 5 |
| Protocol Engine | `discord.js ^12.5.1` (+ 16 monkey-patches) |
| Voice Pipeline | discord.js voice + `prism-media`, `opusscript 0.0.7` (WASM Opus), `tweetnacl` (xsalsa20), PulseAudio (`pactl`) |
| UI Assets | Material Design Icons (41 PNGs), Source Sans Pro |
| Footprint | 128 TS files; TS+SCSS ~13,444 lines |
| Localization | `i18n`, de-DE / en-US / ru-RU (~165 strings per language) |
| Maintenance Status | Dormant (v0.5.1, January 2021; last commit January 2023) |
| License | GPL-3.0 |
| Authentication | User token entry (no direct login credentials); multi-account profiles |
| Plugin System | Node modules with keyword `discord-qt-plugin` in `~/.config/discord-qt/plugins` |

---

## 2. Feature Comparison Matrix

| Domain | discord-qt (v0.5.1) | Clean-Room Reference (`docs/re/`) | Native Rust Client (`discord-gpui`) |
|---|---|---|---|
| **Gateway Protocol** | JSON WebSocket via `discord.js` | ETF + zstd stream, Opcodes 0–43 | WebSocket + ETF + zstd stream, Opcodes 0–43 |
| **Authentication** | Manual token entry | Login, QR auth, 2FA SMS/TOTP, Keyring | Login, QR auth, 2FA, `libsecret` Keyring |
| **Audio Voice** | `opusscript` WASM + `xsalsa20` | WebRTC RTP, AES-256-GCM, Krisp | Native `cpal` + `webrtc`, Opus 48kHz, DAVE MLS v1 |
| **Screen Share** | Not supported | PipeWire ScreenCast + X11 SHM | PipeWire Portal + X11 SHM fallback |
| **Chat & Formatting** | Basic markdown | Full AST, emojis, reactions, replies | Virtualized 100k messages, AST markdown, reactions |
| **Search Engine** | Not supported | REST v9 Search + HTTP 202 backoff | Full-text FTS5 SQLite + server search BFF |
| **Permissions** | Basic role names | 64-bit bitmask calculation (41 flags) | Full 64-bit permission engine v9 |
| **Local Storage** | Flat JSON config | SQLite WAL + FTS5 index | SQLite WAL + FTS5 full-text index |
| **UI Framework** | Qt Widgets (NodeGUI) | Electron / Webpack chunks | GPU-native GPUI (WGPU 120 FPS) |

---

## 3. Conclusions and Key Insights

1. **Protocol Serialization:** Direct ETF (Erlang External Term Format) with zstd streaming delivers substantially lower memory and CPU overhead than JSON.
2. **Media Pipeline:** Native WebRTC audio and video pipelines with PipeWire DMA-BUF buffer sharing provide superior performance compared to Node child-process wrappers.
3. **Architecture:** Clean decoupling of network transport, local storage, and rendering in Rust ensures memory safety (`#![forbid(unsafe_code)]`) and low resource consumption (`VmRSS < 20 MB`).
