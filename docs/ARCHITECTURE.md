# Chord — Architecture & Code Reference

This document provides a technical overview of the **Chord** Rust workspace, module responsibilities, REST/Permissions architecture, local SQLite storage, GPUI component catalog, and internationalization (`chord-i18n`).

---

## 1. Workspace Crates Overview (`rust/crates/`)

All Rust crates enforce `#![forbid(unsafe_code)]` (except isolated FFI boundaries where explicitly audited), `< 50` lines per function, and `< 800` lines per source file.

| Crate | Path | Responsibility |
|---|---|---|
| **`chord-app`** | `rust/crates/chord-app/` | Application bootstrap, runtime wiring, lifecycle orchestration |
| **`chord-core`** | `rust/crates/chord-core/` | Discord Gateway v9, REST API v9 client, rate limiter, permissions, forums, threads, polls, audit logs |
| **`chord-voice`** | `rust/crates/chord-voice/` | WebRTC signaling, UDP/SRTP transport, DAVE MLS v1 (`RFC 9420`), Opus, RNNoise, HRTF, Soundboard |
| **`chord-codec`** | `rust/crates/chord-codec/` | Erlang External Term Format (`ETF`) encoder/decoder (zip-bomb safe) and `zstd-stream` decompression |
| **`chord-media`** | `rust/crates/chord-media/` | Zero-copy audio ring buffers, two-tier LRU media cache (256 MB RAM + disk), Lottie/image decoding |
| **`chord-platform`** | `rust/crates/chord-platform/` | XDG Autostart, StatusNotifierItem tray, D-Bus Secret Service (`org.freedesktop.secrets`), Spotify MPRIS, AT-SPI |
| **`chord-store`** | `rust/crates/chord-store/` | SQLite WAL storage engine, FTS5 full-text message search, schema migrations |
| **`chord-i18n`** | `rust/crates/chord-i18n/` | Localization engine supporting 36 locales, pluralization, relative time, duration, and `xxh64` key hashing |
| **`chord-ui`** | `rust/crates/chord-ui/` | Reusable GPUI UI primitives, dialog windows, and shared layout helpers |
| **`chord-json`** | `rust/crates/chord-json/` | Strict JSON serialization and validation utilities |
| **`chord-perf`** | `rust/crates/chord-perf/` | Frame budget (`120 FPS`), cold-start, and `VmRSS` memory profiling utilities |
| **`chord-updater`** | `rust/crates/chord-updater/` | Release manifest verification and update checking |
| **`chord-conformance`** | `rust/crates/chord-conformance/` | Clean-room protocol conformance harnesses against `docs/re/` oracles |

---

## 2. REST API v9 & Permissions (`chord-core`)

### 2.1. REST Client Modules (`rust/crates/chord-core/src/rest/`)
- **Transport & Framing:** `client.rs`, `transport.rs`, `http1.rs`, `headers.rs`, `super_properties.rs`
- **Resilience & Rate Limiting:** `rate_limit.rs` (bucket-based `X-RateLimit-*` handling), `retry.rs` (exponential backoff on `429` / `5xx`)
- **Domain Endpoints:**
  - `identity.rs`, `user.rs` — authentication, profile, relationships, user settings
  - `guilds.rs` — guild metadata, roles, channels, member lists
  - `ack.rs` — read state & message acknowledgment
  - `search.rs` — remote guild/channel message search
  - `routes.rs`, `resources.rs`, `telemetry.rs` — route definitions and diagnostic helpers

For the full clean-room endpoint catalog, see [`docs/re/rest-d02.md`](re/rest-d02.md).

### 2.2. 64-Bit Permission Bitmask (`rust/crates/chord-core/src/permissions.rs`)
Chord models Discord's 64-bit permission bitfield (`u64`) for channel overwrites and guild roles (`41` documented permission flags exposed in `guild/roles.rs`), computing effective permissions in the standard order:
1. Base `@everyone` guild role permissions
2. Union of all assigned member role permissions
3. Guild-level `everyone` channel overwrite (`deny` then `allow`)
4. Role-specific channel overwrites (`deny` union, then `allow` union)
5. Member-specific channel overwrite (`deny` then `allow`)

`ADMINISTRATOR` (`1 << 3`) and guild ownership bypass channel overwrites per Discord specification.

---

## 3. Local Storage & Encryption (`chord-store` & `chord-gpui/src/storage`)

- **Engine:** SQLite in **WAL (Write-Ahead Logging)** mode (`rust/crates/chord-store/src/lib.rs`, `ffi.rs`) for concurrent reads without blocking the 120 FPS UI thread.
- **Full-Text Search:** Virtual **FTS5** tables index cached messages locally for instant `Ctrl+F` search (`ui/search_drawer.rs`).
- **Encrypted Store (`rust/chord-gpui/src/encrypted_store.rs`):** Sensitive local state is encrypted using authenticated symmetric encryption (`aes-gcm` / `chacha20poly1305`), with master keys anchored in the Linux Secret Service keyring.
- **Rollback & Recovery:** See [`docs/specs/ROLLBACK-PROCEDURE.md`](specs/ROLLBACK-PROCEDURE.md) for local database recovery and schema rollback steps.

---

## 4. Native GPUI UI Component Catalog (`rust/chord-gpui/src/ui/`)

The native interface is rendered directly on GPU via WGPU (`120 FPS` target, `VmRSS < 60 MB` idle budget). Design tokens are defined in [`docs/specs/design-tokens-v2.md`](specs/design-tokens-v2.md) and `rust/chord-gpui/src/theme/`.

| Module | Role |
|---|---|
| `layout.rs` | Root workspace layout, keyboard shortcuts (`Ctrl+K/F/E/V`, `Esc`), and overlay composition |
| `server_rail.rs` | 72px leftmost guild navigation rail, DM button, and server discovery trigger |
| `channel_sidebar.rs` | 240px channel tree (text, voice, forum, stage, categories) and user footer |
| `chat_view.rs` | Virtualized message timeline, composer, Markdown, embeds, and attachments |
| `member_sidebar.rs` | Online/offline/role-grouped guild member list |
| `friends_view.rs` | DM & Friends home view (Online, All, Pending, Blocked tabs) |
| `explore_view.rs` | Public server discovery browser |
| `voice_panel.rs` | Bottom-left voice connection status and mic/deafen/disconnect controls |
| `call_overlay.rs` | Active voice/video call stage and participant tiles |
| `screen_share_modal.rs` | Screen/window/audio capture target selector |
| `quick_switcher.rs` | `Ctrl+K` fast jump modal across guilds, channels, and DMs |
| `search_drawer.rs` | `Ctrl+F` FTS5 message search drawer |
| `emoji_picker.rs` | Standard and custom emoji/sticker picker popover (`Ctrl+E`) |
| `settings_dialog.rs` | 8-tab user & audio/video/theme/accessibility settings modal |
| `login_modal.rs` | Token/credentials and QR authentication modal |
| `captcha_modal.rs` | Interactive CAPTCHA challenge modal |
| `account_switcher.rs` | Multi-account switcher overlay |
| `status_picker.rs` | Presence status selector (Online, Idle, DND, Invisible) |
| `user_profile_card.rs` | Compact user profile popout card |
| `user_profile_sidebar.rs` | DM right-hand user profile panel |
| `create_guild_modal.rs` | New server creation modal |
| `create_channel_modal.rs` | New channel creation modal |
| `context_menu.rs` | Right-click context menu for messages, channels, and users |

---

## 5. Internationalization (`chord-i18n`)

`rust/crates/chord-i18n/` implements Discord-compatible localization across **36 locales** (`docs/re/i18n-deep.md`).

- **Modules (`rust/crates/chord-i18n/src/`):**
  - `locale.rs` — locale parsing and fallback chain
  - `plural.rs` — CLDR pluralization rules
  - `format.rs` / `format_data.rs` — message interpolation
  - `relative.rs` / `relative_data.rs` — relative time formatting (`"2 minutes ago"`)
  - `duration.rs` / `duration_data.rs` — duration formatting
  - `list.rs` / `list_data.rs` — locale-aware list formatting
  - `timestamp.rs` / `timestamp_data.rs` — Discord `<t:...:...>` timestamp rendering
  - `markdown.rs` — i18n-safe inline Markdown formatting

To contribute translations or add new localization keys, follow the workflow in [`CONTRIBUTING.md`](../CONTRIBUTING.md#internationalization-i18n-contributions).
