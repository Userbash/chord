# Discord Gateway Transport Protocol: WS + ETF + zstd-stream

**Cleanroom Reverse Engineering Specification**  
Empirical analysis based strictly on CDP WebSocket frame captures. No live network traffic initiated during analysis.

- **Baseline Fact Snapshot:** `gw-proto/facts-frozen.json`
- **Machine-Readable Schema:** `gateway-protocol.json`
- **Primary Inputs:** `capture-ready.jsonl` (run1), `capture-run2.jsonl` (run2), partial decodes in `gateway/messages.jsonl`, decoder `gw-decode.mjs`.

**Key Architectural Invariants:**
1. Exactly **11 ETF tags** appear in observed traffic: 70, 97, 98, 100, 104, 106, 107, 108, 109, 110, 116 — totaling **486,300** tag occurrences.
2. Snowflake IDs use **big-int** encoding `[110][n:u8][sign:u8][n bytes LE]`: sign precedes digits.
3. `zstd-stream` maintains a single contiguous compression stream per connection; magic bytes `28 b5 2f fd` appear only in frame 0.
4. Inbound Gateway opcodes: **0 (Dispatch), 10 (HELLO), 11 (ACK)**, and **7 (RECONNECT)**.
5. Standalone `GUILD_CREATE` dispatches are not emitted when `data_mode=full`: complete guild data arrives inside `READY.guilds[]`.

---

## 1. Methodology

- WebSocket frame captures recorded via CDP `webSocketFrameSent/Received`. Outbound binary frames are masked as `length + sha256` to prevent token leakage.
- Independent decoder validation:
  1. `gateway/messages.jsonl` (93 terms) aligns as an exact prefix of `.1389` (137 terms) with 0 discrepancies.
  2. Resolves big-int byte ordering: negative ID discrepancies in earlier parsers arose from evaluating sign after numeric payload.
  3. Gateway Snowflake IDs cross-referenced against REST API and CDN URLs.

## 2. ETF Wire Representation and Tag Map

Framing: Uncompressed stream emits `0x83` (ETF version byte) prior to every term. Initial stream payload: `83 74 00 00 00 04 | 64 00 01 64 …` — `MAP(4)` representing HELLO.

| Tag | Code | Type | Occurrences | Field Example |
|---:|---|---|---:|---|
| 70 | 0x46 | NEW_FLOAT_EXT (double BE) | 240 | `term.d.soundboard_sounds[0].volume` |
| 97 | 0x61 | SMALL_INTEGER_EXT (u8) | 32,920 | `term.op` |
| 98 | 0x62 | INTEGER_EXT (i32 BE) | 13,159 | `term.d.heartbeat_interval` |
| 100 | 0x64 | ATOM_EXT (u16 len, latin1) | 243,387 | Term dictionary keys `term.<key>` |
| 104 | 0x68 | SMALL_TUPLE_EXT (u8 count) | 1,113 | `apex_experiments.assignments[0]` |
| 106 | 0x6a | NIL_EXT (null / empty list) | 12,621 | `term.d._trace[nil]`, list terminators |
| 107 | 0x6b | STRING_EXT (u16 len) | 178 | `auth.authenticator_types` |
| 108 | 0x6c | LIST_EXT (u32 count + nil trailer) | 8,794 | `term.d._trace` |
| 109 | 0x6d | BINARY_EXT (u32 len, UTF-8) | 99,030 | String payloads and message content |
| 110 | 0x6e | SMALL_BIG_EXT (u8 n; sign; n bytes LE) | 34,843 | Snowflake IDs (`relationships[0].id`) |
| 116 | 0x74 | MAP_EXT (u32 count) | 40,015 | `term.d` |

### Clarifications on Wire Data:
1. **nil representations:** ATOM `"nil"` (tag 100) accounts for 42,232 instances; NIL_EXT (tag 106) accounts for 12,621 instances (8,794 being LIST_EXT terminators).
2. **Booleans:** Transmitted as ATOM strings `"true"` (6,894) and `"false"` (25,977).
3. **Snowflake Big-Int Invariant:** Format is `[110][n:u8][sign:u8][n bytes LE]`.
   - `guild.id` raw `6e 08 00 14 30 04 f5 cc 5c 14 0e` → **`1014537851418980372`** (1,503 occurrences).
   - `user.id` raw `6e 08 00 00 00 c4 84 c9 4e 55 05` → **`384299970632876032`** (58 occurrences).
   - `channel.id` raw `6e 08 00 47 20 84 11 10 c8 54 10` → **`1176785373989445703`** (31 occurrences).

## 3. Streaming Compression: `zstd-stream`

- Connection endpoint URI: `?encoding=etf&v=9&compress=zstd-stream`.
- **Contiguous zstd context:** The zstd magic header (`28 b5 2f fd`) appears exclusively in frame 0. Subsequent frames are raw streaming delta continuations without headers.
- The decompressor must preserve dictionary context across frames and must not call `end()` between frames.

Socket Compression Metrics:

| Socket ID | Frame Count | Compressed Bytes | Decompressed Bytes | Decoded Terms |
|---|---:|---:|---:|---:|
| run1:57992.5112 | 161 | 121,759 | 946,388 | 161 |
| run2:95974.1389 | 137 | 104,066 | 824,816 | 137 |
| run2:95974.5532 | 109 | 115,891 | 859,251 | 109 |
| run2:95974.8269 | 648 | 86,774 | 898,045 | 648 |
| run2:95974.8608 | 71 | 19,976 | 114,474 | 71 |
| run2:95974.10029 | 374 | 145,255 | 1,289,082 | 374 |
| run2:95974.13072 | 44 | 17,759 | 66,841 | 44 |

## 4. Opcodes, Heartbeat, and Resumption Lifecycle

Inbound Opcode Distribution:

| Socket | Mode | Sequence `s` Range | Opcode 0 | Opcode 10 | Opcode 11 | Opcode 7 | First Dispatch |
|---|---|---|---:|---:|---:|---:|---|
| run1:57992.5112 | Fresh Login | 1..134 | 134 | 1 | 26 | — | READY |
| run2:95974.1389 | Fresh Login | 1..113 | 113 | 1 | 23 | — | READY |
| run2:95974.5532 | Fresh Login | 1..94 | 94 | 1 | 14 | — | READY |
| run2:95974.8269 | Resume #1 | 95..663 | 569 | 1 | 78 | — | GUILD_MEMBER_LIST_UPDATE |
| run2:95974.8608 | Resume #2 | 664..722 | 59 | 1 | 11 | — | GUILD_MEMBER_LIST_UPDATE |
| run2:95974.10029 | Fresh Login | 1..331 | 331 | 1 | 41 | **1** | READY |
| run2:95974.13072 | Resume #3 | 332..371 | 40 | 1 | 3 | — | **RESUMED** |

- Heartbeat interval: `HELLO.d.heartbeat_interval = 41250` ms.
- Reconnection handling: Upon receiving `Opcode 7 (RECONNECT)`, client closes transport socket and initiates RESUME on new connection; sequence `s` transitions cleanly without packet loss.
- Total recorded Opcode 0 Dispatches across sessions: 1,340 events (READY 4, READY_SUPPLEMENTAL 4, RESUMED 3, GUILD_MEMBER_LIST_UPDATE 1048, PRESENCE_UPDATE 107, MESSAGE_CREATE 31, etc.).

## 5. `READY` Payload Schema and Metrics

4 Login Sessions analyzed:
- Top-level keys: **40** properties. User object: 32 properties.
- Entity counts: Guilds (7), Private Channels (17, all type 1 DM), Relationships (60), Read States (157 entries), User Guild Settings (58).
- Payload size: ~579 KB JSON, ~577 KB ETF per instance. Top sections by size: `guilds` (260 KB), `user_guild_settings` (193 KB), `users` (60 KB).

## 6. Guild Invariant (`data_mode=full`)

Under `data_mode: full`, guild structures arrive pre-populated in `READY.guilds[]`, obviating subsequent individual `GUILD_CREATE` dispatches during initial handshake. Reference guild `1014537851418980372` includes 83 channels (47 text, 20 voice, 12 categories, 4 forums), 34 roles, 46 emoji, and 24,229 members.
