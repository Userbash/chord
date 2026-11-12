# Rust Client Task Tree (Freeze B01 Synchronization)

Plan source: `docs/rust-client/PLAN.md` (90 cards). Status markers: `[x]` accepted, `[~]` in review/progress, `[ ]` available, `[!]` blocked.
Status and verification: `docs/rust-client/STATUS.md`; batch plan: `docs/rust-client/WAVE-PLAN.md`; recovery checkpoint: `waves/STATE.md`.

## 1. Current Progress Summary

- Rust workspace: **All tests passing (0 failures)**.
- `discord-core`: 100% contract coverage.
- Full gate passing green across workspace targets.

## 2. Task Allocation Breakdown

### P0 Core Foundation
- [x] P0-01: Workspace scaffolding and crate separation
- [x] P0-02: Snowflake identifier types and JSON serialization
- [x] P0-03: Gateway ETF decoding and term normalization
- [x] P0-04: Gateway zstd streaming decompression
- [x] P0-05: WebSocket transport and Heartbeat lifecycle
- [x] P0-06: Opcode 0 Dispatch router and event channels
- [x] P0-07: READY event hydration and state stores

### P1 State and Protocol Extensions
- [x] P1-01: Channel entity models and permission overwrite computation
- [x] P1-02: Guild entity models and role hierarchy resolution
- [x] P1-03: Message entity models, embeds, and attachments
- [x] P1-04: User profiles, relationships, and presence tracking
- [x] P1-05: Gateway reconnect, resume, and backoff state machine
- [x] P1-06: Member list chunking and windowed synchronization
- [x] P1-07: REST HTTP client, rate limiting, and backoff

### P2 Audio and Media
- [x] P2-01: Voice gateway signaling (Opcode 0..16)
- [x] P2-02: RTP packetization and Opus frame decoding
- [x] P2-03: WebRTC audio device integration and jitter buffering
- [x] P2-04: DAVE E2EE protocol (MLS v1 / RFC 9420)
- [x] P2-05: PipeWire desktop video capture and Wayland integration\n