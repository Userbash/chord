# Disassembling Discord: Rebuilding a High-Performance Client from Scratch

**One developer, autonomous AI agents, and a thousand automated tests. No magic, no VC funding, no corporate bureaucracy.**

---

## The Two-Minute Summary

I took the official Discord desktop client on Linux and completely disassembled its internals to understand its wire protocols, binary serialization formats, and media pipelines. Armed with this clean-room reverse-engineering data, I am rebuilding a native Discord client in Rust.

This is not a CSS theme, a browser script, or an Electron wrapper. It is a completely independent, standalone native application.

The project is called **discord-lab**. It follows a rigorous test-driven architecture:
- Core network engine: WebSocket Gateway v9 with zstd streaming and binary ETF parsing.
- Real-time voice: Native WebRTC, 48 kHz stereo Opus DSP, PipeWire screen sharing, and DAVE MLS v1 end-to-end encryption.
- Direct GPU rendering: 120 FPS interface using GPUI and WGPU, completely eliminating Electron and Chromium.
- Memory consumption: Reduced from 350+ MB (Electron) to **11–18 MB VmRSS** with cold starts under 500 ms.

The codebase is built through autonomous AI engineering loops governed by strict TDD gates, mutation testing (5/5 mutation kill rate per module), and formal dual-review verification.

Below is the technical story of how it was built, the architecture decisions made, and the tools powering the process.

---

## 1. The Genesis: Why Rebuild Discord?

On Linux, Discord runs inside Electron — bundling an entire Chromium browser instance and Node.js runtime. While convenient for cross-platform delivery, the resource cost is immense:
- High idle RAM usage: 300–450 MB just idling in background.
- CPU overhead during voice calls: software audio mixing and DOM reflows.
- Poor Wayland screen sharing support: historical struggles with PipeWire portals and hardware video encoders.

A messaging and voice client should not demand half a gigabyte of memory. I decided to inspect the local installation packages: `app.asar`, `core.asar`, and native C++ shared objects (`discord_voice.node`, Krisp noise reduction, etc.).

Examining the native ELF binaries revealed unstripped symbol tables. Running `nm` across the binaries exposed over 115,000 function signatures: `PacketCrypter::HeaderLength`, `MuteSendStream::SendAudioData`, `TransportMediaContext::SetRemoteDescription`. These symbols became the roadmap for our clean-room reconstruction.

---

## 2. Reverse Engineering Clean-Room Protocols

To construct an independent client, every wire interaction was documented through non-invasive observation:

1. **Gateway WebSocket:** Discord uses binary Erlang External Term Format (ETF) compressed via persistent zstd streams. Decoding this directly in Rust (`discord-codec`) yields sub-millisecond dispatch handling without JSON string allocations.
2. **REST API v9:** 45 core routes mapped with strict headers (`Authorization`, `X-Super-Properties`), rate limits, and exponential backoff retry algorithms.
3. **Voice & Media:** WebRTC SDP negotiation, 74-byte IP Discovery packets, 20ms Opus frames, and SRTP transport encryption (`AES-256-GCM` / `XChaCha20-Poly1305`).
4. **DAVE MLS v1 E2EE:** Complete implementation of Discord's Audio & Video End-to-End Encryption protocol (`RFC 9420`, `DaveHashRatchet`, truncated 8-byte authentication tags, and epoch key rotations).

All findings were preserved in the clean-room reference suite: `docs/re/`.

---

## 3. The Native Rust Stack: Zero Electron

Rather than wrapping a web view, the client architecture was migrated to a 100% native Rust GUI stack:
- **`gpui-kit` & WGPU:** Direct GPU-accelerated drawing at 120 FPS.
- **SQLite with WAL & FTS5:** Local full-text search indexing across tens of thousands of messages in under 5 ms.
- **PipeWire & DMA-BUF:** Native Linux screen sharing with zero-copy hardware buffers and automatic fallback to X11 MIT-SHM.
- **Linux Keyring Integration:** Secure token storage via `libsecret` and D-Bus with automatic secret redaction (`mfa.[REDACTED]`).

---

## 4. Engineering Process: Agents, TDD, and Mutation Testing

Developing a system of this scale required strict software engineering discipline:
- **Strict TDD:** No production code is written without a prior failing test proving the behavior.
- **12 Testing Tiers:** Spanning unit tests, wire contract verification, Playwright E2E journeys, and accessibility audits.
- **Mutation Gate:** Every pull request must pass a 5/5 mutation kill rate to prove that test assertions catch real behavioral defects.
- **Fixed Resource Ceilings:** Supervised worker processes adhere to memory limits (`WORKER_RAM_LIMIT <= 25 GB`), ensuring rock-solid stability.

---

## 5. Conclusion & The Road Ahead

Rebuilding a complex modern desktop application from scratch demonstrates that with clean-room protocol analysis, modern systems programming in Rust, and disciplined agentic engineering, high-performance desktop software can be delivered with unparalleled efficiency: 120 FPS performance, complete end-to-end security, and less than 20 MB of RAM.
