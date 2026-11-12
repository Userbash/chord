# CONTRACTS/dispatch.md — Gateway Dispatch Pipeline

Architecture contract for Gateway Dispatch event ingestion into the Rust client state engine.

## 1. Pipeline Stages

1. **Transport Ingestion:** WebSocket frame reception (`wss://gateway.discord.gg`).
2. **Decompression:** Continuous streaming Zstandard decompression (`discord_zstd`).
3. **Deserialization:** ETF term parser (`discord_erlpack` / native Rust ETF decoder).
4. **Sequence & Opcode Dispatch:** Opcode 0 dispatch routing by event type (`READY`, `MESSAGE_CREATE`, `GUILD_MEMBER_LIST_UPDATE`).
5. **State Reducer:** Pure state transition application with notification triggers.

## 2. Ingestion Contracts

| Event | Reducer Target | State Mutation |
|---|---|---|
| `READY` | `AppState::apply_ready` | Full hydration of guilds, channels, relationships, user |
| `MESSAGE_CREATE` | `AppState::apply_message` | Append message, increment unread / mention counters |
| `MESSAGE_UPDATE` | `AppState::apply_message_update` | In-place update of message content/embeds |
| `GUILD_MEMBER_LIST_UPDATE` | `AppState::apply_member_list` | Windowed member list synchronization |
| `PRESENCE_UPDATE` | `AppState::apply_presence` | Update user status and activity records |

## 3. Concurrency Guarantees

State transitions are processed sequentially per gateway connection to preserve sequence number ordering (`seq`).
Out-of-order events trigger reconnect / resume procedures.\n