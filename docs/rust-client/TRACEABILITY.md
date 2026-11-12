# Traceability: Features -> Specs -> Fixtures -> Tests

This document reconciles the feature catalog (141 features across 14 groups — `FEATURES.md`) with normative specifications
(`docs/re/`), conformance test fixtures (`fixtures/`), and the test catalog
(133 tests across 13 groups — `TEST-PLAN.md`). Methodology: every functional group must maintain
(1) an authoritative source document with concrete figures, (2) an oracle fixture or explicit label "cleanroom lab only",
(3) corresponding tests in the test plan, and (4) assigned implementation cards in `PLAN.md`.

## 1. Group Traceability Matrix

| Functional Group | Count | Specification (docs/re) | Fixtures | Tests | Implementation Waves |
|---|---|---|---|---|---|
| `F-GW` — Event Gateway | 11 | `gateway-protocol.md` §4–6, `gateway-schema.md` | `fixtures/gateway/by-type/`x164, `summary.json`; cleanroom lab traces: `capture-run2.jsonl`, 7 sockets `gw-proto` | `T-GW` 9 (+`T-ENC` 5) | P1, P2, P4 |
| `F-ENC` — Transport Codecs | 3 | `gateway-protocol.md` §2–3 (ETF tags, zstd stream) | `fixtures/gw-proto/facts-frozen.json`, `msgs-run2-…-13072.jsonl` | `T-ENC` 5 | P1 |
| `F-AUTH` — Auth and Sessions | 5 | `gateway-protocol.md` §5 (IDENTIFY/RESUME), `main-arch.md` | cleanroom lab: `capture-ready/boot.jsonl`; `docs/re/gateway-protocol.json` | `T-REST` 9, `T-SEC` 5 | P1, P6, P7 |
| `F-API` — REST v9 Surface | 14 | `rest-d02.md`, `rest-static.json` | `fixtures/rest/rest-static.json`; `docs/re/rest-d02.json` (47 routes); cleanroom lab snapshots | `T-REST` 9 | P1–P3, P6 |
| `F-DATA` — State and Store | 9 | `gateway-schema.md`, `feature-backlog.md` (D) | `fixtures/gateway/by-type/` (GUILD_*, MESSAGE_*), `ready-stats.json` | `T-DATA` 13 | P2, P4, P6 |
| `F-MSG` — Messaging Engine | 12 | `rest-d02.md` §messages, `gateway-protocol.md` §4 | `fixtures/gateway/by-type/` (MESSAGE_*, REACTION); cleanroom lab traces | `T-MSG` 10 | P3, P6, P7 |
| `F-SRCH` — Search and Filter | 5 | `frontend-d03.md` §search | cleanroom lab: `ui*/`, `decoded/`, `frontend-d03.json` | `T-SRCH` 3 | P3 |
| `F-PRES` — Presence and Status | 4 | `gateway-protocol.md` §4 (PRESENCE_UPDATE x107) | `fixtures/gateway/by-type/010-PRESENCE_UPDATE.json`; cleanroom lab traces | `T-DATA` 13 (`T-DATA-11`) | P2, P3, P6 |
| `F-VOICE` — Voice Subsystem | 21 | `voice-deep.md` §1–3 (sessions s2/s3, SRTP, keepalive) | `fixtures/voice/*`; cleanroom lab: `strace-media-s3.jsonl`, `tcpdump*.log` | `T-VOICE` 23 | P4, P4.5, P5, P6 |
| `F-MEDIA` — Video and Streaming | 7 | `voice-deep.md` §4 (codec engine) | cleanroom lab: `engine-log-session1.txt`, pcap | `T-MEDIA` 8 | P3, P5, P7 |
| `F-UI` — User Interface | 24 | `frontend-d03.md` §5 (12 contracts) | `docs/re/frontend-d03.json`; cleanroom lab: `ui*/`, `decoded/` | `T-UI` 21 | P3, P5, P6 |
| `F-I18N` — Internationalization | 7 | `i18n-deep.md` §1–3, §7 (36 codes, 31 enabled) | `fixtures/i18n/i18n-stats.json`; cleanroom lab: ru/en catalogs, `locale-chunk-hashes.json` | `T-I18N` 8 | P3, P7 |
| `F-SYS` — Platform Integrations | 13 | `backend-ipc.md`, `native-re.md` | cleanroom lab: `backend-ipc.json`, `native-re.json` | `T-SYS` 12 | P6, P7, P8 |
| `F-SEC` — Security and Privacy | 6 | `voice-deep.md` §2 (keys), `gateway-protocol.md` §5 | Sanitization across all fixtures; cleanroom lab traces | `T-SEC` 5 | Cross-cutting |
| **Total** | **141** | `docs/re/` (26 files) | `fixtures/` (12 entries) + cleanroom lab | **133** (+ `T-HARN` 7 — harness) | P1–P8 |

The mapping from feature groups to test groups is intentional: `T-REST` covers `F-AUTH` + `F-API`, `T-DATA` covers `F-DATA` + `F-PRES`, and part of `F-GW` is verified in `T-ENC` (codec oracles derived from the same network frames). Sums verified by meta-test `tests/meta/rust-client-sync.test.mjs`.

## 2. Cross-Cutting Verifications

| Check | Location | Verification Mechanism |
|---|---|---|
| Cards P0–P8 -> Tests | `PLAN.md` ("Tests" column of each card) | Meta-test: >=60 cards, waves P0–P8, active links |
| Fixtures -> Tests | `fixtures/MANIFEST.json` (`usedBy` field) | `verify-re.mjs` FIX-01: replica md5 equals manifest |
| Numerical consistency | `docs/re/*.json`, `docs/rust-client/*` | `verify-re.mjs`: 20 automated checks (see `VERIFICATION.md`) |
| Spec -> Product | `docs/re/README.md` (coverage matrix) | Intersection with 14 groups from §1 |
| Sanitization | All test fixtures | `verify-re.mjs` SAN-01; `T-SEC-03` |

## 3. Known Constraints

- Traceability is managed at the **group level**; granular bindings "feature -> specific T-ID" reside inside work cards in `PLAN.md`.
- `F-SEC` is a cross-cutting capability group spanning validation (P1), transport encryption (P4), and security auditing (P8).
- Sensitive raw traffic captures are preserved exclusively in external cleanroom storage; only curated, sanitized test fixtures are checked into git (`PROJECT-STRUCTURE.md` §3).
