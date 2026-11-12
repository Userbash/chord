# Disassembling Discord Piece by Piece — And Rebuilding It From Scratch

**How one developer, autonomous AI agents, and a thousand automated tests are creating an ultra-fast Discord client in native Rust.**

*Project: discord-lab. October–November 2026.*

---

## If You Only Have a Minute

This article documents an engineering effort that completely disassembled the official Discord Linux desktop installation to understand its internals and protocols, reconstructing an independent native client in Rust.

Not a theme. Not a browser script. Not an Electron web wrapper. A standalone native client.

Key Technical Highlights:
- **Direct GPU-Rendered Interface:** Built using `gpui-kit` and WGPU at 120 FPS. Zero Chromium or WebKit.
- **Microscopic Memory Footprint:** Idles at **11–18 MB VmRSS** (compared to 350+ MB for official Electron).
- **Embedded Binary Protocol:** Erlang External Term Format (ETF) decoding with zstd stream compression.
- **Enterprise-Grade Cryptography:** DAVE MLS v1 E2EE (`RFC 9420`) voice and video protection.
- **Native Linux Media Pipeline:** PipeWire DMA-BUF zero-copy screen sharing with automatic X11 fallback.
- **Disciplined Engineering:** Strict TDD, 12 testing tiers, 100% mutation testing kill rate (5/5 mutations killed per module), and automated CI/CD.

Below is an honest, detailed account of the reverse engineering discoveries, architectural challenges, and engineering methodologies powering the project.

---

## 1. Why Disassemble Discord?

Modern desktop communication software has become increasingly resource-heavy. On Linux, Discord bundles Chromium and Node.js via Electron:
- Idle RAM footprint exceeds 300–450 MB.
- Window drawing relies on complex HTML/CSS layout trees and JavaScript execution.
- System integration (global hotkeys, audio loopbacks, screen sharing) relies on brittle native node addons.

Text messaging, voice communication, and screen sharing should not require hundreds of megabytes of RAM. I began examining the local Discord Linux packages: extracting `app.asar`, `core.asar`, and inspecting native ELF `.node` binaries.

A key discovery: the release binaries contained unstripped symbol tables. Running `nm` across the modules revealed over 115,000 function names, exposing exact C++ class names and method signatures. These symbols provided an invaluable clean-room map of internal behaviors, from audio buffer sizing to packet framing.

---

## 2. Reverse Engineering Clean-Room Protocols

By combining static disassembly with non-invasive network inspection, we mapped Discord's wire architecture:

### 2.1. The Gateway WebSocket
- **Transport:** WebSocket connections over TLS.
- **Encoding:** Binary ETF (Erlang External Term Format) combined with zstd persistent dictionary compression.
- **Opcodes:** Opcodes 0 to 43 covering dispatch events, heartbeats, voice state updates, and lazy guild subscriptions.
- **Efficiency:** Processing binary ETF terms directly in Rust avoids billions of string allocations and JSON conversions.

### 2.2. REST API v9
- **Endpoints:** Over 45 core routes mapped with exact query parameters, request schemas, and header requirements (`X-Super-Properties`, `Authorization`).
- **Resilience:** Sophisticated rate-limit handling with exponential backoff on `HTTP 429 retry_after` headers.

### 2.3. Real-Time Voice & WebRTC
- **Signaling:** Voice Gateway v8 over JSON WebSockets with 13.75-second heartbeat intervals.
- **Transport:** UDP audio/video packets with 74-byte IP Discovery handshakes, 20ms Opus frames, and SRTP transport encryption.
- **DAVE MLS v1 E2EE:** Complete implementation of Discord's End-to-End Encryption protocol using `RFC 9420` MLS key ratchets, epoch rotations, and 30-digit privacy verification codes.

All protocol contracts were codified into clean-room specifications in `docs/re/`.

---

## 3. The Native Rust Architecture

To achieve true desktop performance, the architecture avoids any embedded web browser:

1. **`discord-core`:** Headless protocol state machine managing gateway connections, message history, and user rosters.
2. **`discord-voice`:** Low-latency WebRTC media pipeline, Opus DSP, and DAVE MLS cryptography.
3. **`discord-media`:** PipeWire audio loopback and video capture with zero-copy DMA-BUF sharing.
4. **`discord-store`:** High-speed SQLite database in WAL mode with FTS5 full-text indexing.
5. **`discord-gpui`:** GPU-rendered UI using GPUI and WGPU, delivering smooth 120 FPS rendering.

---

## 4. Engineering Quality and Autonomous Agents

Building this infrastructure required strict software verification:
- **Strict TDD:** Tests are written and committed red before any production code is authored.
- **Mutation Testing:** Every pull request must kill 5/5 control mutations, ensuring assertions detect real bugs.
- **Zero-Idle Pipeline:** Parallel worker streams operate without downtime under fixed memory limits (`WORKER_RAM_LIMIT <= 25 GB`).
- **Comprehensive Meta-Tests:** 147 meta-tests ensure documentation, specifications, and code remain synchronized.

---

## 5. Summary

The project demonstrates that modern systems programming in Rust can replace heavyweight web wrappers with blazing-fast, secure native desktop software: 120 FPS graphics, less than 20 MB of RAM, and complete protocol parity.
