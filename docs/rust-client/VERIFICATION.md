# Verification: Cleanroom RE Evidence <-> Documents <-> Fixtures

Verification run ledger (cleanroom lab oracle).
Command: `node docs/rust-client/verify/verify-re.mjs --write` -> **20/20 checks passed**
(machine report — `verify/report.json`). Script resides in git; cleanroom lab acts as authoritative oracle.

## 1. Results

| ID | Verification | Fact (from raw evidence) |
|---|---|---|
| LAB-01 | Cleanroom lab accessibility | `gateway/` and root directories intact |
| PROV-01 | D-02 snapshot md5/byte sizes match `docs/re/rest-d02.stats.json` | run2: `abfe14b76cec…` 4,137,676 B / 10,170 lines; ready: `b43636da09a4…` 2,479,139 B / 5,104 lines — all exact match |
| GW-01 | `gateway/by-type` | 164 files, 12 types |
| GW-02 | READY `002-READY.json` | 40 keys, 7 guilds, 17 DMs, 146 users, 60 relationships, 157 `read_state` |
| GW-03 | Guild ITSKIDON | 83 channels / 34 roles / 46 emojis; `member_count` 24,228 (READY) and 24,229 (facts) — see §2.1 |
| GW-04 | `ready-stats.json` | 7 guilds, 17 DMs, 60 relationships, 40 keys |
| GW-05 | `summary.json` | READY frame `payloadBytes=575853` present |
| GW-06 | `gw-proto` (7 sockets) | 1,544 frames, 1,340 dispatches, 21 types |
| ENC-01 | ETF tags | 11 codes in raw data **=** 11 in `docs/re/gateway-protocol.json` (identical sets) |
| REST-01 | `docs/re/rest-d02.json` | 47 routes, 133 requests v9, `Authorization` — 221 |
| REST-02 | `rest-static.json` | 904 static registry endpoints |
| I18N-01 | `i18n-stats.json` | ru 23,836 keys, en 23,903, plural pairs 911 |
| I18N-02 | `locale-chunk-hashes.json` | 31 enabled locales out of 36 |
| IPC-01 | `backend-ipc.json` | 168 channels, 115 duplex, 35 bridge entries, whitelist 43 (30+14) |
| NAT-01 | `native-re.json` | 9 modules; `discord_voice` 126,136,024 B, md5 `5fe80ce0f5d8…` |
| VOICE-01 | `voice/rtc*-frames.jsonl` | Opcode set = {0,1,2,3,4,6,8,15,16} |
| VOICE-02 | `strace-media-s3.jsonl` | Keepalive magic `1337` present |
| SAN-01 | `capture-run2.jsonl` + `fixtures/` | Zero token-like strings; sensitive keys redacted to `REDACTED` (177 files) |
| FIX-01 | `fixtures/MANIFEST.json` | 12 entries, md5 of all copies match exactly (0 deviations) |
| FIX-02 | Gateway fixture | HELLO heartbeat `41250` present |

## 2. Reconciled Differences

### 2.1 `member_count`: 24,228 vs 24,229
Both values exist in raw evidence: READY decode (`gateway/by-type/002-READY.json`) reports **24,228**, while frozen facts (`gw-proto/facts-frozen.json`, socket `.8269`) report **24,229**. This reflects active membership changes across snapshots rather than an error. Documentation standardizes on 24,229.

### 2.2 "23 dispatch types" -> **21**
Early drafts of `FEATURES.md` / `PLAN.md` / `TEST-PLAN.md` listed 23 types. Verification against the union of 7 `gw-proto` sockets and `docs/re/gateway-protocol.md` §4 confirmed exactly **21 distinct types**. Corrected across all documents and automated in check GW-06.

### 2.3 `MESSAGE_POLL_VOTE_REMOVE`
Initially cataloged alongside `ADD`. In raw data (0 occurrences in `capture-run2.jsonl`), this event was not observed. Updated to "ADD; REMOVE — not observed", with negative control test case in T-MSG-08.

### 2.4 "47 routes" vs "904 endpoints"
`rest-static.json` contains the static endpoint registry scraped from web bundles (**904** endpoints), while "47 routes / 133 requests" represent real captured traffic in trace D-02. Verified separately via REST-01 and REST-02.

### 2.5 Real `analytics_token` Redacted from READY
Security scanning identified an active `analytics_token` in `gateway/by-type/002-READY.json`. The fixture generator (`fixtures/tools/make-fixtures.mjs`) strips `analytics_token`, `session_id`, `auth_session_id_hash`, `static_client_session_id`, `qos_token`, and `resume_gateway_url` -> `REDACTED` before committing to git, verified by check SAN-01.

## 3. Guarantees Provided

- All numerical claims in `docs/re/` and `docs/rust-client/` reproduce automatically from raw evidence (20 checks).
- Conformance fixtures in the repository are byte-exact replicas of verified cleanroom fixtures.
- Sanitization verified across all fixtures by automated scanner SAN-01 and test `T-SEC-03`.
- Architectural integrity protected by meta-test `tests/meta/rust-client-sync.test.mjs`.

## 4. Reproduction

```sh
node docs/rust-client/verify/verify-re.mjs          # 0 = clean; 1 = deviation; 2 = missing lab
node docs/rust-client/verify/verify-re.mjs --write  # update verify/report.json
node fixtures/tools/make-fixtures.mjs               # rebuild fixtures (copy + sanitize + manifest)
```
