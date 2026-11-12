# Fuzz Testing Report — Protocols & DAVE MLS v1 Cryptography

- **Status:** `passed`
- **Generated:** `2026-11-11T10:24:22.365Z`
- **Target iteration threshold per target:** `1000000`
- **Targets verified:** `5`
- **Crashes / leaks found:** `0`

## Fuzz Target Summary

| Target | Iterations | Peak RSS (MB) | Speed (exec/s) | Status |
|---|---:|---:|---:|---|
| `fuzz_aes128_gcm_truncated8` | 1000000 | 509 | 45454 | PASS |
| `fuzz_anti_replay_window` | 1000000 | 509 | 30303 | PASS |
| `fuzz_gateway_json_etf` | 1000000 | 259 | 40000 | PASS |
| `fuzz_rtp_header_extensions` | 1000000 | 504 | 37037 | PASS |
| `fuzz_voice_udp_packets` | 1000000 | 413 | 250000 | PASS |

## Crash & Leak Details

No panics, AddressSanitizer errors (OOB/UAF), or LeakSanitizer memory leaks detected (`0` failures).
