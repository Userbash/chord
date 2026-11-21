# Worker Report — TASK-903: Comprehensive Fuzz Testing of Network Protocols & DAVE MLS v1 Cryptography (`cargo-fuzz` + `AddressSanitizer` / `LeakSanitizer`)

## 1. Task Card

- **Task ID:** `task_59b599d7132f` (`TASK-903`, `TASK-903.1`, `TASK-903.2`, `TASK-903.3`)
- **Dispatch ID:** `ctx_4015bde17ef4`
- **Status:** Completed (`5 × 1,000,000 = 5,000,000` `libFuzzer` + `ASan` + `LSan` iterations, `0` panics, `0` OOB, `0` memory leaks).

## 2. Work Completed

1. **Protocol & cryptographic primitives (`#forbid(unsafe_code)`, Strict TDD):**
   - `discord-codec`: `ZstdStream` protection against streaming RLE zip-bombs (`WindowLogMax(24)`, `MAX_DECOMPRESSED_CHUNK_BYTES = 16 MiB`, `MAX_PENDING_COMPRESSED_BYTES = 8 MiB`).
   - `discord-voice`: strict 74-byte IP Discovery and 8-byte Keepalive codecs (`0x1337cafe` / `0x1337f00d`), RTP extension parser/encoder RFC 5285 / RFC 8285 One-Byte (`0xBEDE`) and Two-Byte (`0x1000..=0x100F`), constant-time streaming `Aes128GcmTruncated8` (no lookup tables, no heap allocations in `ghash`), 1024-bit sliding `AntiReplayWindow` on `[u64; 16]`, and modular pipeline `dave_v1::{replay, mls, session}` (`DaveMlsKeySchedule`, `DaveHashRatchet`, `DaveFrameEncryptor`, `DaveFrameDecryptor`, `DaveVoiceSession` with `op:27 Proposal` and `op:29 Commit` verification).
   - `chord-gpui`: fixed overflow and boundary defects in `dave_compute_wrapped_generation`, `DaveHashRatchet::get`, and `DiscordVoicePacket::parse_ip_discovery_response`.

2. **`rust/fuzz` package (`cargo-fuzz` / `libFuzzer` + `AddressSanitizer`) and minimized corpus `tests/fuzz/corpus/<target>/`:**
   - `fuzz_gateway_json_etf`: incoming Gateway messages (`discord-json`, `discord-codec::etf`, `ZstdStream`, `GatewayPipe`, `Session`, `route_dispatch`, `apply_frame`, `discord-voice::parse_frame`).
   - `fuzz_voice_udp_packets`: 74-byte IP Discovery, 8-byte Keepalive, and compound `rtcp::parse_compound` packets.
   - `fuzz_rtp_header_extensions`: RTP headers, `0xBEDE` and `0x1000..=0x100F` extensions, `restore_rtx_bytes`, and `VideoFrameAssembler` (H.264 / AV1).
   - `fuzz_aes128_gcm_truncated8`: `Aes128GcmTruncated8` on arbitrary/truncated/corrupted buffers, Opus frame cipher `0xFAFA`, and corrupted MLS Proposals / Commits in `DaveVoiceSession` and `DaveMlsEngine`.
   - `fuzz_anti_replay_window`: `AntiReplayWindow` and `dave_compute_wrapped_generation` on `0` and `u64::MAX` overflows, verifying each step against the reference oracle `HashSet<u64>`.

3. **Nightly CI and auto-report infrastructure (`TASK-903.3`):**
   - `scripts/fuzz-report.mjs` + unit tests `tests/unit/fuzz-report.test.mjs` (classification of `panic`, `asan_oob`, `memory_leak`, `timeout_or_oom`, SHA-256 and hex preview of crash artifact, reproduction command).
   - `scripts/fuzz-ci.sh` (sequential offline/CI runner with `-rss_limit_mb=2048` and `--cmin` controls).
   - `.github/workflows/fuzz-nightly.yml` (nightly pipeline `cron: "0 3 * * *"` + `workflow_dispatch` with upload of `.gate/fuzz-*` and `rust/fuzz/artifacts/**`).

## 3. Real Defects Found & Fixed (4 total)

1. **`discord-codec::zstd::ZstdStream::push` — RLE zip-bomb:** An 806-byte RFC 8878 RLE frame decompressed to `26,214,400` bytes (`25 MiB`) in a single call with no memory ceiling. Fixed with `MAX_WINDOW_LOG = 24` and `MAX_DECOMPRESSED_CHUNK_BYTES = 16 MiB` limits (`tests/zstd_bomb_contract.rs`).
2. **`chord-gpui::audio::dave_compute_wrapped_generation` — panic overflow:** Multiplication `(wraps + 1) * 256` overflowed `u32` when `oldest >= 0xFFFF_FF01`. Fixed via modular difference `wrapping_sub` + `saturating_add` (`tests/dave_fuzz_regressions.rs`).
3. **`chord-gpui::audio::DaveHashRatchet::get` — infinite loop at `u32::MAX`:** `saturating_add(1)` left `next_generation == u32::MAX`, causing hang/OOM. Added `exhausted` flag and bounded loop (`tests/dave_fuzz_regressions.rs`).
4. **`chord-gpui::audio::DiscordVoicePacket::parse_ip_discovery_response` — weak boundary validation:** Buffers `> 74` bytes, `len != 70`, `port == 0`, and non-zero trailing bytes after `NUL` were accepted. Introduced strict validation of all 74-byte packet invariants (`tests/dave_fuzz_regressions.rs`).

## 4. Red/Green Summary & 1,000,000 Iterations ASan + LSan

| Fuzz Target | Iterations | Peak RSS (MB) | Speed (`exec/s`) | Status |
|---|---:|---:|---:|---|
| `fuzz_aes128_gcm_truncated8` | `1,000,000` | `509` | `45,454` | `PASS` |
| `fuzz_anti_replay_window` | `1,000,000` | `509` | `30,303` | `PASS` |
| `fuzz_gateway_json_etf` | `1,000,000` | `259` | `40,000` | `PASS` |
| `fuzz_rtp_header_extensions` | `1,000,000` | `504` | `37,037` | `PASS` |
| `fuzz_voice_udp_packets` | `1,000,000` | `413` | `250,000` | `PASS` |

- **Targeted / Contract / Lint gates:**
  - `cargo test -p discord-codec`: `48/48` green.
  - `cargo test -p discord-voice`: all unit + contract tests (including `crypto_vectors` and `fuzz_protocol_dave_contract`) green.
  - `cargo test --manifest-path rust/discord-gpui/Cargo.toml --test dave_fuzz_regressions --test gpui_contract_test`: `13/13` and `127/127` green.
  - `cargo clippy -p discord-codec -p discord-voice --all-targets -- -D warnings` and `cargo clippy --manifest-path rust/fuzz/Cargo.toml --all-targets -- -D warnings`: `0` warnings.
  - `node --test tests/unit/fuzz-report.test.mjs`: `3/3` green (after fixing RED `ERR_MODULE_NOT_FOUND`).
  - `node --test tests/meta/ci-browser.test.mjs tests/meta/rust-client-sync.test.mjs tests/meta/fixtures-tracked.test.mjs`: `25/25` green.

## 5. Files & MD5

- `9c965f959d16b14a57eaff7a1b58f54b` `rust/crates/discord-codec/src/zstd.rs`
- `664bfbc96538936fb8ad5549589cdcd2` `rust/crates/discord-codec/tests/zstd_bomb_contract.rs`
- `338a00116454f98f857289e9dfd8584c` `rust/crates/discord-voice/src/lib.rs`
- `8ad71dbabd5846804a14e59fe6da0110` `rust/crates/discord-voice/src/udp.rs`
- `cbe7854bf1173b3cea2eb4a9922dee4e` `rust/crates/discord-voice/src/media/rtp.rs`
- `6cead7815409a5d1a7685613a3e0c4c4` `rust/crates/discord-voice/src/crypto/mod.rs`
- `e4c39fbf787abe67dd3b3bd32e09d99f` `rust/crates/discord-voice/src/crypto/aes.rs`
- `a580c6639d6d8fc18a43549be53a636a` `rust/crates/discord-voice/src/crypto/gcm.rs`
- `a4e7873601a78ebf3bfee67203e2fea8` `rust/crates/discord-voice/src/dave_v1/mod.rs`
- `8e83b6b4617d8dfafa3df842301eb3a3` `rust/crates/discord-voice/src/dave_v1/replay.rs`
- `94838d96e2e469de2e287f4f25ed0b8f` `rust/crates/discord-voice/src/dave_v1/mls.rs`
- `f68110751667e37caabda8911266f962` `rust/crates/discord-voice/src/dave_v1/session.rs`
- `4f41dec3a4e908f6bccd77feb5fe8d32` `rust/crates/discord-voice/tests/fuzz_protocol_dave_contract.rs`
- `cea90c2f689cd0fe2fce25bc574c372f` `rust/discord-gpui/src/audio/mod.rs`
- `7f63989ad7f31e0c472334601dd8d672` `rust/discord-gpui/tests/dave_fuzz_regressions.rs`
- `ce861ccf40de57560efbfb4a89e86d44` `rust/fuzz/Cargo.toml`
- `b42c874b7248c7b57c580c2e7c1a56b3` `rust/fuzz/fuzz_targets/fuzz_gateway_json_etf.rs`
- `ce816c0df487afe045c912459f4dd903` `rust/fuzz/fuzz_targets/fuzz_voice_udp_packets.rs`
- `2dd7b3cdb639d0b4612494abadc9c706` `rust/fuzz/fuzz_targets/fuzz_rtp_header_extensions.rs`
- `dca06f40b9578acf760526e5fade1161` `rust/fuzz/fuzz_targets/fuzz_aes128_gcm_truncated8.rs`
- `5d80a5c689baf7ce68e3d731da7e729a` `rust/fuzz/fuzz_targets/fuzz_anti_replay_window.rs`
- `074adffa50151e036ac2c1086d918149` `scripts/fuzz-report.mjs`
- `2e03221f1321bee8bab22c0c9389e3f7` `scripts/fuzz-ci.sh`
- `5d7e7a02a545c6bb8942ec9fe592bd81` `tests/unit/fuzz-report.test.mjs`
- `0aeb0bc45825c4be8121d5fd4645a500` `.github/workflows/fuzz-nightly.yml`
- Minimized corpora: `tests/fuzz/corpus/{fuzz_gateway_json_etf,fuzz_voice_udp_packets,fuzz_rtp_header_extensions,fuzz_aes128_gcm_truncated8,fuzz_anti_replay_window}/`
- Fuzz summary report: `.gate/fuzz-report.md` and `.gate/fuzz-report.json`

## 6. Risks & Integration Notes for Team Lead

Per `AGENTS.md` §1.2 and `TDD-PLAN.md` §8 — not edited by the worker. On batch merge, the coordinator should add a row for `tests/unit/fuzz-report.test.mjs` (`analyzeFuzzRun`, `renderMarkdownReport`) to `TDD-PLAN.md` §8 to keep `plan-sync.test.mjs` in sync.
