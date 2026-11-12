# CONTRACTS/read-state.md — Ack/Read-State in UI (M-07)

Date: 2026-10-24. Owners: G1 (backend, `read_state`/`ack_message`), G2 (UI).
Sources: `fixtures/gateway/by-type/002-READY.json`, `fixtures/gateway/by-type/046-MESSAGE_ACK.json`, `docs/re/rest-d02.md` section 4.

## 1. Data Schema

`READY.read_state.entries[]` -> IPC `read_state()` (camelCase, 9 fields):

```json
{
  "channelId": "string",
  "flags": "number | null",
  "lastMessageId": "string | null",
  "lastPinTimestamp": "string | null",
  "lastViewed": "number | null",
  "mentionCount": "number | null",
  "badgeCount": "number | null",
  "lastAckedId": "string | null",
  "readStateType": "number | null"
}
```

- `lastViewed` — read-state sequence counter (e.g. `4287` in fixture), distinct from snowflake message ID.
- `mentionCount` — unread mention counter; `flags`, `lastViewed`, `mentionCount` are nullable.
- `lastAckedId` — optional snowflake of the last acknowledged message.

## 2. Unread State Calculation

Channel unread state predicate:
`isUnread = (channel.last_message_id != null) && (channel.last_message_id > read_state.last_acked_id)` (Snowflake numeric comparison).

Badge calculation:
`hasBadge = mentionCount != null && mentionCount > 0`.

## 3. Acknowledgement Dispatch

When channel view gains focus and renders latest message:
1. UI dispatches `ack_message(channel_id, message_id)`.
2. Backend submits `POST /api/v9/channels/{channel_id}/messages/{message_id}/ack` with payload:
   `{"token": null, "last_viewed": <counter>}`.
3. Local state updates optimistic read marker.\n