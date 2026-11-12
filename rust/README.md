# Chord — Native Rust Client Architecture & Protocol Layer

Cargo workspace with crate boundaries defined according to clean-room reverse-engineering specifications.

All network protocols, encodings, and voice transport layers are developed independently from clean-room behavioral specifications.

## Crate Responsibilities

| Crate | Responsibility | Specification |
|---|---|---|
| `discord-codec` | ETF (erlpack) binary codec + zstd streams | `docs/re/gateway-protocol.md` §2–3 |
| `discord-core` | Gateway v9, REST v9, session management, reconnection | `docs/re/gateway-protocol.md` §4–6 |
| `discord-store` | Local SQLite WAL cache and encrypted data store | `docs/feature-backlog.md` |
| `discord-voice` | RTC signaling, SRTP, DAVE MLS v1 E2EE, keepalive | `docs/re/voice-deep.md` |
| `discord-media` | Zero-copy audio buffers, Opus/AV1 capture & playback | `docs/re/voice-deep.md` §4 |
| `discord-platform` | System integration (tray, XDG autostart, Keyring, D-Bus) | `docs/re/backend-ipc.md` |
| `discord-i18n` | Localization keys, pluralization, 36 locales | `docs/re/i18n-deep.md` |
| `discord-ui` | Design tokens, components, contracts | `docs/re/ui-rest-d03.md` |
| `discord-gpui` / `chord-gpui` | Native GPUI/WGPU 120 FPS desktop application | `docs/specs/GPUI-ARCHITECTURE-MIGRATION.md` |
| `discord-conformance` | Protocol test harnesses and fixtures | `docs/rust-client/TEST-PLAN.md` §3 |

## Running Tests

```sh
cd rust && cargo test --offline
```

## Legal and Trademark Notice

Chord is an independent, clean-room open-source project and is **not affiliated with, endorsed by, or sponsored by Discord Inc.**
The official Discord client was utilized solely as a local reference for protocol and media pipeline behavioral interoperability under clean-room engineering principles.
"Discord" is a registered trademark of Discord Inc. All trademarks remain the property of their respective owners.
