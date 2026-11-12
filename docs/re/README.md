# Deep Reverse Engineering of Discord (Wave R7) — Artifact Index

Normative documents for Wave R7: transport and gateway, media/voice, backend (main process, IPC, native modules), REST, UI, and i18n. Extracted from an **isolated client copy** (CDP 9223; live client untouched). Secrets (tokens/sessions/keys) sanitized, public IPs masked as `<PUB_IP>`, outbound WS frames recorded only by length + sha256.

Repository boundaries: **normative specifications and machine-readable summaries** (this directory); raw evidence (pcap, strace, decodes, screenshots, sanitized captures) reside in `clean-room-oracle/` or `docs/re/`. Golden test suites (`logs/`, `functions/`, `evidence/`) remain untouched. Wave tasks in `TASKS.md` §R7; graphs in `architecture.md`; backlog status in `../feature-backlog.md` (§D), project map in `../map.md`. Consumer of results: Rust client plan `../rust-client/` (concept, features, architecture, plan P0–P8, tests; tasks in `TASKS.md` §RC).

Prior to git commit, all copies underwent thorough inspection: `user_settings_proto`, `qos_token`, and `analytics_token`, as well as example `settings-proto` payloads, are replaced with `REDACTED`; long base64 strings and token patterns verified to ensure zero leaks.

## 1. Composition

| File | Contents | Key Metrics |
|---|---|---|
| `gateway-protocol.md` + `.json` | Gateway: ETF, zstd stream, opcodes, heartbeat/resume, READY, GUILD_CREATE (D-01) | 11 factual ETF tags; 7 sockets; 1340 dispatches; heartbeat 41,250 ms |
| `gateway-schema.md` + `.json` | Wire schemas: READY, GUILD_MEMBER_LIST_UPDATE, MESSAGE_CREATE/UPDATE, HELLO/ACK | 40 keys in READY; rhythm 41.25 s |
| `rest-d02.md` + `.json` + `.stats.json` + `.crossref.json` + `rest-d02-verify.sh` | D-02: REST route registry, payloads, auth flow, super-properties | 47 routes, 133 v9 requests; auth — 221 requests / 6 headers; 22 super-property fields; verify exit 0 |
| `voice-deep.md` + `.json` | Voice: RTC signaling, UDP structures, SRTCP, socket rotation | ops 8/0/1/2/4/15/16/3/6; discovery 74 B; keepalive 8 B / 5 s; rotation +60.08 s |
| `backend-ipc.md` + `.json` | IPC Map: `DISCORD_*` channels, `DiscordNative` bridge, call-sites | 168 channels (152 main / 129 renderer / 115 bidirectional); whitelist 43; bridge 35; 47 call-sites |
| `main-arch.md` + `.json` | main process: bootstrap, windows/`webPreferences`, 5 full IPC chains, native bindings, updater | 121/121 citations verified by offset |
| `native-re.md` + `.json` (+ `native-modules.md`/`.json`) | 9 native modules: sizes, md5, NAPI tables, dlopen, Frida inventory | `discord_voice.node` 126.1 MB (`5fe80ce0…`), `discord_krisp.node` 62.1 MB (`9be929d1…`) |
| `frontend-d03.md` + `.json` | D-03: UI traces (search, emoji, members, settings, popover) + component contracts | 12 contracts; "ProBot" → 3302; ":fire" → 10 |
| `i18n-deep.md` + `.json` (+ `i18n-stats.json`) | i18n: custom runtime, key-hash, pluralization, rich markup, locales | key = xxh64 LE → base64[0..6] (36 bits); 31/36 locales; 23,836 keys |
| `architecture.md` | Mermaid graphs: processes, IPC chains, bridge, native modules, gateway, voice, REST | — |

## 2. Parity Against Initial Matrix

| Domain | Prior State | Current State | Coverage Source |
|---|---|---|---|
| Transport/gateway (ETF, zstd, heartbeat/resume) | ~90% | **≈97%** | `gateway-protocol.*` (with big-int correction) |
| Voice crypto | ~95% | **≈95%** | `voice-deep.md` §2.3–2.4 |
| Voice signaling | ~85% | **≈95%** | 3 live sessions + engine logs, `voice-deep.md` §1 |
| Media RTP/RTCP, jitter buffer | ~85% | **≈90%** | `voice-deep.md` §2, lab `strace-media-s3.jsonl` |
| REST client | ~60% | **≈90%** | `rest-d02.*`, sanitized capture |
| UI kit / component contracts | ~90% | **≈95%** | `frontend-d03.md` §5 (12 contracts) |
| Screens: chat, DM, emoji, popovers | ~55–65% | **≈90%** | `frontend-d03.*` |
| i18n (pluralization/rich) | ~80% | **≈95%** | `i18n-deep.*` |
| Backend (main/IPC/native) | — | **complete** | `main-arch.*`, `backend-ipc.*`, `native-re.*` |

## 3. Key Findings Summary

- **D-01 (READY/GUILD_CREATE) Closed:** 4 complete READY payloads: 4 logins, 40 keys, 7 guilds, 17 DMs, 60 relationships, 146 users, 157 read_states. GUILD_CREATE is not dispatched when `data_mode: full` is active: all guilds are hydrated in `READY.guilds[]`.
- **D-02 (REST) Closed:** From real capture — 47 routes in `/api/v9`, POST/PUT/PATCH bodies, auth flow (headers, `X-Super-Properties` 22 fields, build 641004).
- **D-03 (UI) Closed:** Search (7 filter sections, sorting), emoji picker (3 tabs, 10 categories), members, settings, user popover; 12 component contracts with variants/tokens/quotes.
- **Voice:** Complete RTC-WS signaling, UDP byte structures (discovery/keepalive/RTP/SRTCP), media socket rotation at +60.08 s, DTX and comfort noise, `aead_aes256_gcm_rtpsize` mode, DAVE MLS v1 in Opcode 4.
- **Backend:** 5 complete IPC chains `DISCORD_*`, 168 channels, `DiscordNative` bridge, 9 native modules (totaling 239 MB), dlopen dependencies (PipeWire/Vulkan/FFmpeg), updater, windows and `webPreferences`, feature flags, crashpad, deep link/tray.
- **i18n:** Custom runtime: key = `xxh64(LE bytes) → base64[0..6]` (36 bits), CLDR pluralization, 23 rich-text markup rules, 31 of 36 locales.
- **Gateway:** 11 factual ETF tags, big-int encoding (sign precedes digits), zstd compression magic on frame 0, `op7 RECONNECT → RESUME → RESUMED` without sequence breaks.

## 4. Methodology

| Tool/Method | Deliverable |
|---|---|
| Spec-compliant ETF big-int + Snowflake cross-ref with REST/URL | Verified Snowflake IDs in decodes |
| Incremental streaming zstd decoder (without `end()`) | 100% frame recovery across 7 WS sockets |
| strace → C-escape `iov_base` decode from `sendmsg/recvmsg` | Full UDP bytes during truncated pcap captures |
| tcpdump `-s 96` (snap) + custom packet analyzers | Lightweight pcap captures (2.6 MB vs 46 MB) |
| CDP drivers with trusted `Input.*` + sentinel files | Clean UI traces without intrusive JS clicks |
| `ss -unap` at voice connection | Media socket owner identification (pid/fd/port) |
| Verification workers (every quote mapped to file offset) | 121/121 main-arch citations verified |
| JSONL sanitizer (keys + inline secrets) | Git-compliant sanitized artifacts |

## 5. Reproduction (Cleanroom Environment)

```bash
cd docs/re
node gw-decode.mjs < capture-ready.jsonl
bash rest-d02-verify.sh
node voice/rtp-analyze.mjs voice/voice-udp3.pcap 104.29.147.145 voice-udp3.rtp.json
node voice/strace-extract-packets.mjs voice/strace-renderer-s3.log 104.29.146.60 voice/strace-media-s3.jsonl
```

Full consumer architecture available in [rust-client](../rust-client/).
