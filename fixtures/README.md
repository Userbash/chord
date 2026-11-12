# Reverse-Engineering Fixtures (Conformance Suite)

A curated set of RE Wave (R7) artifacts used to verify the codecs, gateway schema, REST layer,
i18n, and voice stack of the Rust client (`docs/rust-client/TEST-PLAN.md` §3).

- Raw material is **read-only**: files are byte-for-byte copies from the `reference cleanroom artifacts` lab
  (except for redacted secrets, see below).
- Provenance of each file (source path, size, md5, purpose, tests) is recorded in `MANIFEST.json`.
- The full lab (pcap, strace, dumps, locale catalogs) is available only in `reference cleanroom artifacts`;
  this directory contains the minimal slice sufficient for L1–L3 tests.
- **Sanitization:** values of `analytics_token`, `session_id`, `auth_session_id_hash`,
  `static_client_session_id`, `qos_token`, `resume_gateway_url` (and email) → `REDACTED`.
  The READY decode (`gateway/by-type/002-READY.json`) originally contained a **real
  analytics_token** — stripped on copy (`docs/rust-client/VERIFICATION.md` §2.5).
  Build and sanitization scan: `tools/make-fixtures.mjs`; verification: SAN-01 verifier.

| Fixture | Description | Tests |
|---|---|---|
| `gateway/by-type/` (164 files) | Decoded dispatches by type (12 types) | T-GW-08, T-DATA-06 |
| `gateway/ready-stats.json` | READY counters: 7 guilds, 17 DMs, 60 connections | T-GW-04, T-DATA-02 |
| `gateway/summary.json` | Socket frame summary (op/t/s/sizes) | T-GW-01, T-ENC-04 |
| `gw-proto/facts-frozen.json` | Frozen facts: ETF tags (11), counters | T-ENC-02, T-GW-08 |
| `gw-proto/msgs-run2-95974_13072.jsonl` | Decoded frames from a single socket session | T-ENC-04, T-GW-02 |
| `rest/rest-static.json` | Static REST registry (904 endpoints) | T-REST-01, T-REST-09 |
| `i18n/i18n-stats.json` | i18n: 23,836 ru-keys, modules, locales | T-I18N-01, T-I18N-07 |
| `voice/rtc*-{frames,summary}` | RTC signaling: frames and summaries (sessions 1–2) | T-VOICE-01…03 |
| `voice/session3.timing` | Media socket rotation timings (session 3) | T-VOICE-06 |

## Commands

```sh
node fixtures/tools/make-fixtures.mjs            # rebuild from lab (targeted; does not touch README/tools)
node docs/rust-client/verify/verify-re.mjs       # verify: md5, counters, constants, sanitization (SAN-01)
```
