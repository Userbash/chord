# Rust Client Test Plan: Strict TDD, Conformance, and Cleanroom Oracles

This document defines the comprehensive testing strategy and **catalog of automated tests** driving implementation in the Rust client. The governing development methodology is strict Test-Driven Development (TDD): tests are authored and proven failing (red) prior to code implementation (`PLAN.md`, gate 4). Conformance test vectors are derived from the reference cleanroom oracle `clean-room-oracle/` (thoroughly sanitized, with zero credentials).

## 1. Testing Levels (L0–L7)

| Level | Name | Verification Scope | Location |
|---|---|---|---|
| L0 | Static | Code formatting, linter warnings, type checking: `cargo fmt --check`, `clippy -D warnings`, `tsc` | Pre-commit / CI gate |
| L1 | Unit | Pure functions, ETF/zstd parsing, i18n hashing, SRTP framing, pluralization | In-tree (`#[cfg(test)]`) |
| L2 | Conformance | Byte-exact cleanroom RE fixtures -> verified against expected schema oracle | `discord-conformance` crate |
| L3 | Integration | Local loopback mock servers (WebSocket, REST, WebRTC); multi-step journey scenarios | `crates/*/tests` integration binaries |
| L4 | System / E2E-smoke | Manual verification against reference sandbox; authentication, voice streaming | Manual QA checklist (`smoke.md`) |
| L5 | Fuzz | Parser resilience and crash resistance (ETF, zstd, RTP/SRTP, Protobuf) | `cargo-fuzz` targets |
| L6 | Mutation | Mutation testing: kill rate, survivor/INCONCLUSIVE triage, exact restore via sha256 | `waves/<ID>/run-mutations*.py` |
| L7 | Acceptance | Non-functional contour: performance budgets (ARCHITECTURE §9), security audit, ADR sign-off | Wave acceptance gate |

Operational Rules:
- L0, L1, and L2 are mandatory for every Pull Request.
- L3 is enforced on integrated batch gates.
- L4 is strictly gated by operator authorization (never run unattended against live production accounts in CI).
- L5 runs in scheduled nightly fuzzing jobs.
- L6 is mandatory for every code-bearing wave.
- L7 represents formal milestone and release acceptance (non-functional verification, security review, and risk sign-off).

### 1.0 Selection Matrix

The test level selection matrix determines mandatory testing requirements by change category:

| Change Category | Mandatory Test Levels | Notes and Guidance |
|---|---|---|
| Documentation / Plan Only | L0 | Meta-tests enforcing documentation synchronization and cross-links |
| Small Bugfix (S) | L0, L1, L6 | 3–5 mutation test injections to verify assertion sensitivity |
| Module / Codec Parser (M) | L0, L1, L2, L6 | Cleanroom fixture oracle verification mandatory |
| Integration Feature (L/XL) | L0–L3, L6 | Mock transport servers, end-to-end journey tests |
| UI & Platform Feature | L0–L3, L6, L7 | Non-functional performance budgets and security review |
| Final Release Promotion | L0–L7 | Full testing contour, operator acceptance, and signed release |

Oracle Requirements:
Every test must assert expected behavior against an independent test **oracle** (cleanroom capture artifact, jq/awk/sha256 specification, or external reference vector) rather than asserting against the current code implementation.

### 1.1 Test Budgets and Execution Velocity

Testing budgets prevent redundant assertions while ensuring complete contractual coverage. Target budget per task card:

| Card Size | Test Count | Mutation Budget | Guidance |
|---|---:|---:|---|
| S | 6–10 | 3–5 | Single module or parser; 2–3 boundary conditions |
| M | 10–16 | 5–8 | Data models + reducer/selectors; core operational branches |
| L | 16–24 | 8–12 | Multi-module interactions; stale/duplicate/boundary guards |
| XL | 24–32 | 10–14 | Reserved for complex views (chat list, media pipelines); subdivide where possible |

Optimization Rules:
- **Targeted Execution:** Workers run only targeted test binaries for assigned modules; full workspace runs occur once per integrated batch.
- **Table-Driven Tests:** Parametrized test cases verify boundary variants without duplicating boilerplate test functions.
- **Single Oracle per Behavior:** Avoid redundant assertions across multiple architectural layers if an underlying layer already guarantees the invariant.
- **Skip-Aware Handling:** Environmental absences produce explicit skips with documented reasons; skips never count as passes.

### 1.2 Gap Discovery and Defect Prevention Cycle

Before freezing task contracts, coordinators inspect recent wave reports and `waves/STATE.md`. Any identified risk from code review, surviving mutation, or intermittent failure must be addressed by an assigned test ID, a dedicated task card, or documented justification.

Every functional behavior is verified across all applicable dimensions: standard execution path, lower/upper boundaries and off-by-one, empty/invalid/truncated payloads, duplicate keys, error paths and resource cleanup, reentrancy, descriptor ownership, concurrency, and environmental failure modes.

Mutation test sets derive from real fault models: condition removal/inversion, shifted boundary offsets, modified bytes/ordering, ignored errors, disabled cleanups, and lifted capacity caps. A mutation is classified as `KILLED` only when the targeted test fails on a clean baseline; compiler errors or timeouts yield `INCONCLUSIVE`.

## 2. Fixture Manifest (`clean-room-oracle/`)

| Test Domain | Artifact Paths | Conformance Verification Purpose |
|---|---|---|
| Gateway & Decoders | `gateway/by-type/*.json` (001-nil, 002-READY, 005-GUILD_MEMBER_LIST_UPDATE...), `gateway/messages.jsonl`, `gateway/summary.json`, `gw-proto/*.jsonl` | ETF/zstd oracle: raw byte frames -> verified against normalized JSON |
| READY & Session Traces | `capture-ready.jsonl`, `capture-run2.jsonl` (sanitized), `capture-boot.jsonl` | Identification, resume sequencing, counter integrity |
| REST API v9 | `rest-static.json` (47 routes + static), `rest-static.md`, `d02-snapshot/` | Route paths, request headers, payload schemas, HTTP status codes |
| Voice Engine | `voice/rtc*-frames.jsonl`, `voice/rtc*-summary.json`, `voice/strace-media-s3.jsonl`, `voice/tcpdump*.log`, `voice/LEAVE`, `voice/session3.timing` | Signaling handshakes, UDP/SRTP/RTCP binary layouts, socket rotation timings |
| Voice Tools | `voice/strace-extract-packets.mjs`, `voice/rtp-analyze.mjs`, `voice/hex-offsets.mjs` | Independent verification via reference packet analysis scripts |
| Localization | `i18n-stats.json`, `en-catalog.json`, `catalog-inventory.json`, `locale-chunk-hashes.json` | Hash key functions, CLDR pluralization rules, translation catalogs |
| User Interface | `ui*/`, `decoded/`, `frontend-d03.json` (`../re/frontend-d03.md`) | Component state contracts, modal layouts, design tokens |
| Redaction Verification | All fixtures pre-sanitized with `REDACTED` placeholders | Validated by automated secret scanner `T-SEC-03` |

The repository contains a **curated subset** of this manifest in `fixtures/` (12 entries, 1.59 MB; tracked by provenance file `fixtures/MANIFEST.json`).

## 3. Test Catalog

Format: **ID** | Level | Verification Scope | Reference Oracle / Fixture

### Transport Codecs (T-ENC)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-ENC-01 | L1 | ETF: all 11 wire tags, round-trip serialization | `../re/gateway-protocol.md` §2 specification |
| T-ENC-02 | L1 | ETF big-int: little-endian byte ordering, sign flag preceding digits | Reference vectors in `gateway-protocol.md` §2 |
| T-ENC-03 | L1 | ETF: nested maps, lists, atoms, nil tokens, snowflake strings | `gateway/by-type/001-nil.json` |
| T-ENC-04 | L2 | zstd-stream: frames 2+ without magic header bytes; frame boundaries | `gw-proto/msgs-run2-*.jsonl` |
| T-ENC-05 | L5 | Fuzz ETF & zstd decoders: zero panics, memory bounds enforced | `cargo-fuzz` generators |

### Gateway Engine (T-GW)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-GW-01 | L3 | HELLO handshake -> 41,250 ms heartbeat cycle -> ACK handling | Loopback WebSocket mock server |
| T-GW-02 | L2 | IDENTIFY: validate opcode 2 fields against 943-byte reference payload | `gateway-run1/`, `capture-ready.jsonl` |
| T-GW-03 | L3 | Missed heartbeat ACK triggers socket termination and reconnect | Loopback mock server |
| T-GW-04 | L2 | READY payload schema: 40 top-level keys, entity counts | `gateway/by-type/002-READY.json`, `ready-stats.json` |
| T-GW-05 | L2 | RESUME: sequence number continuity across reconnections | `capture-run2.jsonl` |
| T-GW-06 | L3 | Opcode 7 RECONNECT -> issue RESUME -> receive RESUMED with zero lost events | Loopback mock server |
| T-GW-07 | L3 | 3s disconnect triggers RESUME; 30s disconnect triggers fresh IDENTIFY | Loopback mock server |
| T-GW-08 | L2 | Dispatch router: 21 event types mapped; unhandled events logged raw | `gateway/by-type/` fixtures |
| T-GW-09 | L2 | Opcode 37 subscriptions `[[0, 99]]` and member list mutation ops | `005/006-GUILD_MEMBER_LIST_UPDATE.json` |

### REST Client (T-REST)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-REST-01 | L2 | 47 REST endpoints: HTTP methods, route paths, request payloads | `rest-static.json` |
| T-REST-02 | L2 | 6 authentication headers, status codes 200/204/304/404/429 | `rest-d02.json` |
| T-REST-03 | L3 | Request timeouts and backoff retries on idempotent GET requests | Mock HTTP server |
| T-REST-04 | L1 | `X-Super-Properties`: 22 fields, base64 JSON round-trip | `../re/rest-d02.md` §6.2 |
| T-REST-05 | L3 | HTTP 429 respects `Retry-After`; backoff prevents request storms | Mock HTTP server |
| T-REST-06 | L3 | HTTP 401 triggers session invalidation and returns to login view | Mock HTTP server |
| T-REST-07 | L2 | Message ACK: payload `{token, last_viewed}`; batching 5 requests | `rest-d02.json` §4 |
| T-REST-08 | L2 | Message history: pagination limits, `before`, `after` query parameters | `rest-d02.json` §2 |
| T-REST-09 | L2 | Ecosystem endpoints (quests, entitlements, library) are read-only | `rest-static.json` |

### Data & State (T-DATA)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-DATA-01 | L2 | READY: parse large guild with 83 channels, 34 roles, 46 emojis, 24,228 members | `gateway/by-type/002-READY.json` |
| T-DATA-02 | L2 | Verify 60 relationships, 1 active session, `user_settings_proto` present | `gateway/by-type/002-READY.json` |
| T-DATA-03 | L2 | Parse 17 direct message channels and 146 initial user records | `gateway/by-type/002-READY.json` |
| T-DATA-04 | L2 | State reducer determinism: identical inputs produce identical state | Snapshot fixtures |
| T-DATA-05 | L3 | SQLite schema migrations: lossless migration from v1 to v2 | Migration test runner |
| T-DATA-06 | L2 | Member roster update operations: SYNC, INSERT, UPDATE, DELETE | `005/006-GUILD_MEMBER_LIST_UPDATE.json` |
| T-DATA-07 | L3 | Lazy member chunking: fetch visible viewport members on demand | Loopback mock server |
| T-DATA-08 | L3 | Message history pagination: scroll backfill without duplicate entries | Loopback mock server |
| T-DATA-09 | L2 | Read state tracking: 157 channels, `MESSAGE_ACK` updates | `capture-run2.jsonl` |
| T-DATA-10 | L1 | User settings decoding: decode 4,400-byte Protobuf settings payload | `../re/rest-d02.md` §4 |
| T-DATA-11 | L2 | `PRESENCE_UPDATE`: user statuses and rich activities (107 events) | `capture-run2.jsonl` |
| T-DATA-12 | L2 | Voice channel participant states and call start times in server tree | `capture-run2.jsonl` |
| T-DATA-13 | L2 | Typing state: `TYPING_START` dispatch with indicator auto-timeout | `capture-run2.jsonl` |

### Messaging & Composer (T-MSG)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-MSG-01 | L1 | Markdown rendering: 23 formatting rules, HTML entity escaping | `../re/i18n-deep.md` §3 |
| T-MSG-02 | L1 | Message composer: mentions, emoji shortcuts, Enter / Shift+Enter | Unit tests |
| T-MSG-03 | L3 | Attachment upload: progress monitoring, cancellation, retry | Mock HTTP server |
| T-MSG-04 | L2 | Emoji reactions: add, remove, reaction counters, user list | Mock HTTP server |
| T-MSG-05 | L2 | Message actions menu: verify at least 12 standard action items | `../re/frontend-d03.md` §5.4 |
| T-MSG-06 | L2 | Quote replies and navigation to original message | Unit & Mock tests |
| T-MSG-07 | L2 | Discussion threads: `THREAD_LIST_SYNC` (7), thread sidebar | `capture-run2.jsonl` |
| T-MSG-08 | L2 | Native polls: `MESSAGE_POLL_VOTE_ADD` real-time vote updates | `capture-run2.jsonl` |
| T-MSG-09 | L2 | Custom stickers and animated GIFs: insertion into composer | Mock HTTP server |
| T-MSG-10 | L2 | Custom status updates: text, emoji, and expiration timestamp | Mock HTTP server |

### Search & Discovery (T-SRCH)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-SRCH-01 | L2 | 7 search filter tokens, filter chips, date sorting | `../re/frontend-d03.md` §1 |
| T-SRCH-02 | L3 | Message search: `total_results`, pagination, keyword highlighting | Mock HTTP server |
| T-SRCH-03 | L3 | Cleanroom bug regression test: channel filter chip must not return total 0 | Mock HTTP server |

### Voice Engine (T-VOICE)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-VOICE-01 | L2 | Voice signaling opcodes: 8, 0, 1, 2, 4, 15, 16, 3, 6 validation | `voice/rtc*-frames.jsonl` |
| T-VOICE-02 | L2 | Voice Identify: `max_dave_protocol_version: 1`, stream descriptors | `voice/rtc1-frames.jsonl` |
| T-VOICE-03 | L2 | Voice Ready & Session Description: encryption modes, 32-byte secret key | `voice/rtc*-summary.json` |
| T-VOICE-04 | L1 | UDP IP discovery (74 bytes): type, SSRC, ASCII IP, port BE@72 | `voice/hex-offsets.mjs` |
| T-VOICE-05 | L1 | NAT keepalive (8 bytes): `1337cafe` / `1337f00d` every 5 seconds | `voice/tcpdump*.log` |
| T-VOICE-06 | L2 | Port rotation at +60.08s: discovery -> new port -> empty RR packet | `voice/session3.timing`, `strace-media-s3.jsonl` |
| T-VOICE-07 | L1 | SRTP-GCM: 16-byte authentication tag + 4-byte LE counter tail | Reference vectors in `voice-deep.md` §2.3 |
| T-VOICE-08 | L1 | XChaCha20-Poly1305 encryption mode + client/server counters | Reference vectors in `voice-deep.md` §2.4 |
| T-VOICE-09 | L2 | RTP Opus (PT 120, 20ms): monotonic seq/ts increments, 0xBEDE extension | `voice/tcpdump*.log` |
| T-VOICE-10 | L2 | Discontinuous Transmission (DTX): ~10 silence packets -> comfort noise | `voice/tcpdump*.log` |
| T-VOICE-11 | L3 | Jitter buffer resilience: clear audio output under 5% packet loss | WebRTC mock harness |
| T-VOICE-12 | L3 | Audio device hotplug: device disconnect triggers clean fallback | System mock harness |
| T-VOICE-13 | L3 | Push-to-Talk (PTT): global hotkey takes precedence over voice activation | Unit & system tests |
| T-VOICE-14 | L2 | Voice & Video settings tab: 38 controls bound to audio state | `../re/frontend-d03.md` §4.2 |
| T-VOICE-15 | L2 | Mute, deafen, and speaking indicator state machine transitions | WebRTC mock harness |
| T-VOICE-16 | L1 | RTCP parser: Sender Report (60B), Receiver Report (52B), empty RR (28B) | Reference vectors in `voice-deep.md` §2.4 |
| T-VOICE-17 | L1 | Neural noise suppression: -20 dB background noise reduction | Curated audio test vectors |
| T-VOICE-18 | L2 | Soundboard: `SOUNDBOARD_SOUNDS` (21) audio playback | `capture-run2.jsonl` |
| T-VOICE-19 | L3 | Voice region UDP ping probes and automatic optimal node ranking | WebRTC mock harness |
| T-VOICE-20 | L3 | Connection resilience: network switch and sleep wake auto-rejoin | WebRTC mock harness |
| T-VOICE-21 | L2 | DAVE MLS framing: Welcome, Commit, and Application message formatting | DAVE MLS RFC specification |
| T-VOICE-22 | L3 | OpenMLS group key schedule and participant ratchets on join/leave | 3-participant test harness |
| T-VOICE-23 | L4 | E2EE interoperability: dual-client encrypted audio call verification | Manual QA testbed |

### Media & Video (T-MEDIA)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-MEDIA-01 | L3 | Screen capture via PipeWire / X11: monitor and window selection | System portal mock |
| T-MEDIA-02 | L1 | Hardware encoder: keyframe interval, bitrate controls, frame sizing | Encoder benchmark harness |
| T-MEDIA-03 | L2 | RTP video packetizer: RID and quality layers, RTX retransmission | Reference test vectors |
| T-MEDIA-04 | L3 | Go Live screen share: stream descriptor `rid: 100`, quality adaptation | WebRTC mock harness |
| T-MEDIA-05 | L3 | Webcam capture via V4L2: preview generation and stream transmission | System device mock |
| T-MEDIA-06 | L2 | Attachment media player: format detection and dimensions validation | Unit tests |
| T-MEDIA-07 | L3 | Desktop audio loopback: system audio mixing via PipeWire monitor | System audio mock |
| T-MEDIA-08 | L3 | Inbound video decoding: concurrent participant feeds without frame drops | Video test harness |

### User Interface (T-UI)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-UI-01 | L1 | UI toolkit spike: 3 screens, FPS/RAM/build benchmarks recorded | ADR-1 |
| T-UI-02 | L1 | Member virtualization: smooth 60 FPS scrolling with 24,229 members | READY capture fixture |
| T-UI-03 | L2 | Server rail: badge counters, notification pips, drag-and-drop sorting | READY capture fixture |
| T-UI-04 | L2 | Channel tree: category folding, unread highlights, voice channels | Cleanroom UI fixtures |
| T-UI-05 | L2 | Direct messages list: 17 private channels with activity status | READY capture fixture |
| T-UI-06 | L2 | Chat view: author grouping, timestamp formatters, day dividers | Component snapshots |
| T-UI-07 | L2 | Message virtualization: 100k messages scrolling with low memory footprint | Benchmark harness |
| T-UI-08 | L2 | Member roster panel: role hierarchy grouping, online/offline sorting | READY capture fixture |
| T-UI-09 | L2 | Emoji picker: 3 tabs, 10 categories, 50 visible items, search filter | `../re/frontend-d03.md` §2 |
| T-UI-10 | L2 | QuickSwitcher: global keyboard-driven fuzzy navigation | Unit tests |
| T-UI-11 | L2 | Accessible menus: default/danger item styling, roving tabindex | `frontend-d03.md` §5.3 |
| T-UI-12 | L2 | Modal dialogs: focus traps, ARIA dialog roles, Escape dismissal | `frontend-d03.md` §5.5 |
| T-UI-13 | L2 | Popover stacking: portal rendering, click-outside dismissal | `frontend-d03.md` §5.6 |
| T-UI-14 | L2 | Form controls: switches, sliders, tooltips, avatars, scrollbars | `frontend-d03.md` §5.7–5.12 |
| T-UI-15 | L1 | Design tokens: dark/light, compact/cozy; 5,132 token mapping | `frontend-d03.md` §6 |
| T-UI-16 | L2 | Sound effects: 15 distinct interface audio cues | Audio asset catalog |
| T-UI-17 | L2 | Global keyboard shortcuts: Ctrl+K, Ctrl+F, Ctrl+E, Esc | Unit tests |
| T-UI-18 | L2 | Settings dialog: 8 categories, 38 Voice & Video controls | `frontend-d03.md` §4 |
| T-UI-19 | L1 | Performance benchmarks meeting ARCHITECTURE §9 requirements | CI metric runner |
| T-UI-20 | L2 | Accessibility: screen reader semantics and keyboard accessibility | WCAG compliance audit |
| T-UI-21 | L2 | Edge cases: empty states, error placeholders, skeleton loaders | Component snapshots |

### Internationalization (T-I18N)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-I18N-01 | L1 | Hash key algorithm: `xxh64(LE bytes) -> base64 -> first 6 chars` | `i18n-stats.json` |
| T-I18N-02 | L1 | Locale selection and deterministic fallback to `en-US` | Localization catalogs |
| T-I18N-03 | L2 | Core string resolution across English and Russian catalogs | `en-catalog.json` |
| T-I18N-04 | L2 | Russian CLDR pluralization rules (`one`, `few`, `many`, `other`) + `=N` | `../re/i18n-deep.md` §2 |
| T-I18N-05 | L2 | Rich text markup: 23 formatting tags with proper escaping | `i18n-deep.md` §3 |
| T-I18N-06 | L1 | Cultural number and date formatters per active locale | Unit tests |
| T-I18N-07 | L2 | Catalog coverage across 31 languages; dynamic language switching | `catalog-inventory.json` |
| T-I18N-08 | L2 | Argument interpolation in pluralized strings (e.g. member counts) | Translation catalogs |

### Platform Integration (T-SYS)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-SYS-01 | L3 | System tray: context menu, minimize-to-tray, notification blinking | Platform API mock |
| T-SYS-02 | L3 | Desktop notifications: FreeDesktop D-Bus specification, action buttons | D-Bus mock service |
| T-SYS-03 | L2 | Launcher unread badge counter and XDG autostart file generation | Desktop file validator |
| T-SYS-04 | L2 | Deep link URI handling: `discord://` routing, single-instance IPC | Unit tests |
| T-SYS-05 | L3 | Clipboard access: image paste directly uploads into composer | System clipboard mock |
| T-SYS-06 | L3 | Power management inhibitors: sleep inhibition during active calls | Systemd-logind mock |
| T-SYS-07 | L2 | Native spellchecking: Hunspell dictionary lookups and underlines | Hunspell dictionary tests |
| T-SYS-08 | L2 | Multi-session sync: `SESSIONS_REPLACE` dispatch and popout windows | Session capture fixtures |
| T-SYS-09 | L3 | Account switching: complete cache purging and clean reconnect | State harness |
| T-SYS-10 | L3 | Video clips: recorded gameplay clip management and playback | Filesystem mock |
| T-SYS-11 | L3 | In-app auto-updater: Ed25519 signature checks and atomic rollback | Staged update mock |
| T-SYS-12 | L3 | Multi-platform backends: Windows and macOS compatibility matrix | Platform CI runners |

### Security & Privacy (T-SEC)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-SEC-01 | L1 | Keyring vault: secure store, retrieve, delete; headless encrypted fallback | Secret Service mock |
| T-SEC-02 | L1 | Credential scanner: auth tokens never appear in disk logs or dumps | CI log scanner |
| T-SEC-03 | L1 | Test fixture scanner: zero unmasked secrets in repository fixtures | Fixture audit scanner |
| T-SEC-04 | L3 | Network isolation: telemetry endpoints (`/science`, `/beaker`) blocked | Loopback mock server |
| T-SEC-05 | L2 | WebAuthn and hardware security key multi-factor authentication | WebAuthn test vectors |

### Harness & Quality Operations (T-HARN)

| ID | Level | Verification Scope | Reference Oracle |
|---|---|---|---|
| T-HARN-01 | L2 | Fixture loader discovers `clean-room-oracle/` with version contract | Fixture manifest |
| T-HARN-02 | L2 | Replay gateway event stream: raw frames match normalized JSON | `gw-proto/` captures |
| T-HARN-03 | L2 | Replay REST traffic: N sequential requests reproduced deterministically | `rest-d02.json` |
| T-HARN-04 | L1 | Performance regression gate: >10% slowdown triggers build failure | Benchmark history |
| T-HARN-05 | L2 | Packaging builds: Flatpak, AppImage, and Debian packages start headless | CI container runner |
| T-HARN-06 | L2 | CI pipeline: format, clippy, cargo-deny, unit, and benchmark steps | CI workflow |
| T-HARN-07 | L2 | Supply chain audit: `cargo-deny` passes cleanly, SBOM generated | SBOM verification script |

## 4. Red-First Implementation Protocol

1. For each task card in `PLAN.md`, write the corresponding test(s) from §3 **first**.
2. Run the targeted test suite and capture the verifiable failure log (`cargo test <test_name> -- --exact`).
3. Author the minimal implementation required to turn the test green; never modify test assertions to match incorrect implementation.
4. Capture both the red and green verification logs in the wave task report (`PROCESS.md` §4.2).
5. Intermittent failures or flakiness are never suppressed: rerun and record all attempt logs.

## 5. Sanitization and Privacy Rules

- All test fixtures in the repository are strictly sanitized: tokens (`analytics_token`, `qos_token`, `user_settings_proto`) and sensitive bodies are replaced with `REDACTED` placeholders (`../re/README.md`).
- Public IP addresses are masked as `<PUB_IP>`; tests utilize local mock endpoints.
- Production accounts are never connected in automated CI; live smoke tests require explicit operator approval.
- Automated secret scanning (`T-SEC-02` / `T-SEC-03`) runs in CI prior to publishing artifacts.

## 6. Project Meta-Test Alignment

`tests/meta/rust-client-sync.test.mjs` verifies that architecture plans and test catalogs remain synchronized: documentation files exist, Mermaid graphs render valid syntax, feature and test IDs are unique, and cross-references from `README.md`, `TASKS.md`, and `docs/map.md` resolve cleanly.
