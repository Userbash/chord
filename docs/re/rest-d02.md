# D-02 — Discord REST Client: Methods, Payloads, Auth Flow (Live Data)

Source: Two CDP captures recorded on a live Discord 1.0.162 instance (Linux, Electron 42.11.12).
Task: Complete D-02 against **live** empirical data and cross-verify with the static map (`rest-static.md`, 904 records).

## 0. Quantitative Summary

| Metric | Value |
|---|---|
| `req` events in snapshot | 6654 |
| Unique requestId values | 6651 (duplicates: 3) |
| API requests (only /api/v9, `api` + `api-auth`) | 133 |
| Unique API endpoints | **47** |
| Endpoints with request body (postData) | **6** |
| Endpoints with captured response body | **0** |
| Total captured response bodies | 200 (all `/assets/*.js`) |
| Requests with Authorization header | 221 |
| Filtered as noise | 6521 of 6654 (98.0 %) |
| Static cross-verification | 44/47 endpoints confirmed by static analysis (method + path) |

## 0.1 Evidence Index (for Spot Verification)

| Assertion | Where to Inspect (requestId / Command) |
|---|---|
| Super properties (base64 JSON, 22 fields) | `run2|95974.2323, run2|95974.3486, run2|95974.3741` |
| Lowercase `authorization` header (3x) | `run2|95974.3434, run2|95974.8036, ready|57992.7364` |
| `{token,last_viewed}` payload in ack | `run2|95974.3831, run2|95974.4057, run2|95974.8164` |
| `{}` payload in `PUT guilds/{id}/members/@me` | `ready|57992.6989, run2|95974.7409` |
| `{allow_triggers}` payload | `run2|95974.3423, run2|95974.7586, ready|57992.7308` |
| `{message_ids:[…]}` payload | `run2|95974.8279` |
| Truncated settings-proto body (4001 chars) | `run2|95974.8275` |
| Telemetry `{token, events}` | `run2|95974.2417 (science), run2|95974.8268 (beaker)` |
| 304 (conditional GET) | `run2|95974.3428, run2|95974.7594, ready|57992.7316` |
| 404 (negative capability probes) | `run2|95974.2324, run2|95974.7477, ready|57992.7075 / run2|95974.3431, run2|95974.7590, ready|57992.7312 / run2|95974.3801, run2|95974.7989` |
| 429 rate limit | `run2|95974.8271` |
| Voice `op:0` Identify with token+session_id (text WS) | `run2|95974.8158, ready|57992.7568` |
| Main gateway (binary zstd/ETF frames -> READY unreadable) | `run2|95974.1389, run2|95974.5532, ready|57992.5112` |
| Response bodies = strictly /assets/*.js | `bash rest-d02-verify.sh -> section 8 (body non-asset=0)` |
| All numerical values in report | `bash rest-d02-verify.sh -> rest-d02.verify.txt` |

## 1. Methodology and Reproducibility

```
Input files (read-only):
  d02-snapshot/capture-run2.snap.jsonl
    md5=abfe14b76cec1e559c3b45522bc44b7a  bytes=4137676  lines=10170
  d02-snapshot/capture-ready.snap.jsonl
    md5=b43636da09a4a2c0f08d36b1ce66a46a  bytes=2479139  lines=5104
  snapshot captured: 2026-10-19T11:51:47+05:00
  capture-run2.jsonl is a LIVE file, appended during analysis (see U-10),
  so all metrics bind to snapshots in d02-snapshot/, not live file names.
Post-snapshot commands: node rest-d02.mjs && node rest-d02-report.mjs
```

- Parsing: `rest-d02.mjs` — line-by-line JSONL parsing, join `req` <-> `res` on pair `(file, requestId)`.
- Path normalization: 16-21 digit numeric segment -> `{id}`, UUID -> `{uuid}`, other numbers -> `{num}`; query stripped into distinct dictionary.
- Classification: `api` (into main table), `telemetry-*`, `static-assets`, `cdn-media`, `data-url`, `cloudflare-challenge`, `web-navigation`, `third-party` (into Noise section).
- Independent verification: `bash rest-d02-verify.sh` — metrics computed via `jq` + `sort` + `uniq` + `md5sum` + `join` **outside of report parser code**; full output in `rest-d02.verify.txt`. 0 discrepancies with `rest-d02.json`.
- Oracle rationale: Snapshot MD5 + independent line/reqId/code counts in jq. Request payloads quoted as-is (sanitized), requestId indicated for every sample.

Capture limitation: `hb` events in snapshot report `bodiesTaken:200` — hard ceiling of the capture script, not a property of the client.

## 2. Endpoints Table (/api/v9)

All requests targeted `https://discord.com/api/v9/*`. `count` is the number of `req` events in snapshot.
`status` lists codes from `res` matching the requestId (empty = response missed by capture).

| # | Method | Path | Calls | Status Codes | Request Body (keys) | Auth Header | Static |
|---|---|---|---|---|---|---|---|
| 1 | GET | `/channels/{id}/messages` | 11 | 200x11 | — | Authorization | ✔ |
| 2 | GET | `/applications/public` | 7 | 200x7 | — | Authorization | ✔ |
| 3 | GET | `/games/autocomplete` | 6 | 200x6 | — | Authorization | ✔ |
| 4 | POST | `/channels/{id}/messages/{id}/ack` | 5 | 200x5 | `token, last_viewed` | Authorization | ✔ |
| 5 | GET | `/quests/@me` | 5 | 200x5 | — | Authorization | ✔ |
| 6 | GET | `/users/{id}/profile` | 4 | 200x4 | — | Authorization | ✔ |
| 7 | GET | `/users/@me/billing/subscriptions` | 4 | 200x4 | — | Authorization | ✔ |
| 8 | GET | `/content-inventory/users/@me` | 3 | 200x3 | — | Authorization | ✔ |
| 9 | GET | `/games/detectable/exclusions` | 3 | 304x3 | — | Authorization | ✔ |
| 10 | GET | `/partner-sdk/storefront-config` | 3 | 200x3 | — | Authorization | ✔ |
| 11 | GET | `/promotions` | 3 | 200x3 | — | Authorization | ✔ |
| 12 | GET | `/quests/decision` | 3 | 200x3 | — | Authorization | ✔ |
| 13 | GET | `/quests/get-decisions` | 3 | 200x3 | — | Authorization | ✔ |
| 14 | GET | `/users/@me` | 3 | 200x3 | — | authorization | ✔ |
| 15 | GET | `/users/@me/achievements/unread-state` | 3 | 200x3 | — | Authorization | ✔ |
| 16 | GET | `/users/@me/activities/statistics/applications` | 3 | 200x3 | — | Authorization | ✔ |
| 17 | GET | `/users/@me/applications/{id}/entitlements` | 3 | 200x3 | — | Authorization | ✔ |
| 18 | GET | `/users/@me/billing/checkout-recovery` | 3 | 200x3 | — | Authorization | ✔ |
| 19 | POST | `/users/@me/billing/user-offer` | 3 | 204x3 | `allow_triggers` | Authorization | ✔ |
| 20 | GET | `/users/@me/collectibles-marketing` | 3 | 200x3 | — | Authorization | ✔ |
| 21 | GET | `/users/@me/entitlements` | 3 | 200x3 | — | Authorization | ✔ |
| 22 | GET | `/users/@me/library` | 3 | 200x3 | — | Authorization | ✔ |
| 23 | GET | `/users/@me/mfa/webauthn/credentials` | 3 | 200x3 | — | Authorization | ✔ |
| 24 | GET | `/users/@me/referrals/eligibility` | 3 | 404x3 | — | Authorization | ✔ |
| 25 | GET | `/users/@me/saved-messages` | 3 | 404x3 | — | Authorization | ✔ |
| 26 | GET | `/users/@me/settings-proto/{num}` | 3 | 200x3 | — | Authorization | ✔ |
| 27 | GET | `/users/@me/virtual-currency/balance` | 3 | 200x3 | — | Authorization | ✔ |
| 28 | GET | `/widget-configs/developer` | 3 | 200x3 | — | Authorization | ✔ |
| 29 | GET | `/widget-configs/featured` | 3 | 200x3 | — | Authorization | ✔ |
| 30 | GET | `/activities/shelf` | 2 | 200x2 | — | Authorization | ✔ |
| 31 | GET | `/guilds/{id}/application-command-index` | 2 | 200x2 | — | Authorization | method? |
| 32 | PUT | `/guilds/{id}/members/@me` | 2 | 204x2 | `(empty-object)` | Authorization | ✔ |
| 33 | GET | `/soundboard-default-sounds` | 2 | 200x2 | — | Authorization | ✔ |
| 34 | GET | `/users/{id}/application-identities` | 2 | 200x2 | — | Authorization | ✔ |
| 35 | GET | `/users/@me/application-command-index` | 2 | 200x2 | — | Authorization | ✔ |
| 36 | GET | `/users/@me/notes/{id}` | 2 | 404x2 | — | Authorization | ✔ |
| 37 | POST | `/beaker` | 1 | 204x1 | `token, events` | Authorization | no |
| 38 | PATCH | `/channels/{id}/explicit-media` | 1 | 204x1 | `message_ids` | Authorization | ✔ |
| 39 | PUT | `/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me` | 1 | 204x1 | — | Authorization | method? |
| 40 | GET | `/channels/{id}/messages/pins` | 1 | 200x1 | — | Authorization | ✔ |
| 41 | GET | `/channels/{id}/threads/search` | 1 | 200x1 | — | Authorization | ✔ |
| 42 | GET | `/collectibles-products/{id}` | 1 | 200x1 | — | Authorization | ✔ |
| 43 | GET | `/games` | 1 | 200x1 | — | Authorization | ✔ |
| 44 | GET | `/storefront/products/skus` | 1 | 200x1 | — | Authorization | ✔ |
| 45 | GET | `/users/@me/billing/payment-sources` | 1 | 200x1 | — | Authorization | ✔ |
| 46 | PATCH | `/users/@me/settings-proto/{num}` | 1 | 200x1 | `(non-json)` | Authorization | ✔ |
| 47 | GET | `/users/@me/survey` | 1 | 429x1 | — | Authorization | ✔ |

Total: 133 requests across 47 unique routes. Response status codes: 200, 204, 304, 404, 429.

Status code distribution:

| Code | Count | Notes |
|---|---|---|
| 200 | 113 | OK |
| 204 | 8 | Success No Content (ack, user-offer, beaker, explicit-media) |
| 304 | 3 | Conditional GET (cache `games/detectable/exclusions`) |
| 404 | 8 | Negative feature capability probes (`referrals/eligibility`, `saved-messages`, `notes/{id}`) |
| 429 | 1 | Rate limit on `users/@me/survey` (single occurrence) |

Non-200 details:

| Endpoint | Status | requestId |
|---|---|---|
| `GET /games/detectable/exclusions` | 304 (3x) | run2|95974.3428 |
| `POST /users/@me/billing/user-offer` | 204 (3x) | run2|95974.3423 |
| `GET /users/@me/referrals/eligibility` | 404 (3x) | run2|95974.2324 |
| `GET /users/@me/saved-messages` | 404 (3x) | run2|95974.3431 |
| `PUT /guilds/{id}/members/@me` | 204 (2x) | run2|95974.7409 |
| `GET /users/@me/notes/{id}` | 404 (2x) | run2|95974.3801 |
| `POST /beaker` | 204 (1x) | run2|95974.8268 |
| `PATCH /channels/{id}/explicit-media` | 204 (1x) | run2|95974.8279 |
| `PUT /channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me` | 204 (1x) | run2|95974.8046 |
| `GET /users/@me/survey` | 429 (1x) | run2|95974.8271 |

## 3. Query Parameters

Total 45 unique parameter names observed. Most frequent:

| Parameter | Occurrences | Seen On |
|---|---|---|
| `application_ids` | 58 | `GET /api/v9/applications/public`x58 |
| `visible_guild_ids` | 21 | `GET /api/v9/quests/decision`x21 |
| `limit` | 16 | `GET /api/v9/channels/{id}/messages`x11, `GET /api/v9/users/@me/billing/subscriptions`x3 |
| `platform` | 6 | `GET /api/v9/promotions`x3, `GET /api/v9/users/@me/collectibles-marketing`x3 |
| `placement` | 6 | `GET /api/v9/quests/decision`x3, `GET /api/v9/quests/get-decisions`x3 |
| `client_heartbeat_session_id` | 6 | `GET /api/v9/quests/decision`x3, `GET /api/v9/quests/get-decisions`x3 |
| `client_ad_session_id` | 6 | `GET /api/v9/quests/decision`x3, `GET /api/v9/quests/get-decisions`x3 |
| `q` | 6 | `GET /api/v9/games/autocomplete`x6 |
| `filter_group` | 6 | `GET /api/v9/games/autocomplete`x6 |
| `locale` | 5 | `GET /api/v9/promotions`x3, `GET /api/v9/collectibles-products/{id}`x1 |
| `country_code` | 4 | `GET /api/v9/users/@me/library`x3, `GET /api/v9/storefront/products/skus`x1 |
| `type` | 4 | `GET /api/v9/users/{id}/profile`x3, `PUT /api/v9/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me`x1 |
| `with_mutual_guilds` | 4 | `GET /api/v9/users/{id}/profile`x4 |
| `with_mutual_friends` | 4 | `GET /api/v9/users/{id}/profile`x4 |
| `with_mutual_friends_count` | 4 | `GET /api/v9/users/{id}/profile`x4 |
| `with_sku` | 3 | `GET /api/v9/users/@me/entitlements`x3 |
| `with_application` | 3 | `GET /api/v9/users/@me/entitlements`x3 |
| `entitlement_type` | 3 | `GET /api/v9/users/@me/entitlements`x3 |
| `exclude_ended` | 3 | `GET /api/v9/users/@me/entitlements`x3 |
| `exclude_consumed` | 3 | `GET /api/v9/users/@me/applications/{id}/entitlements`x3 |
| `bookmark_ids` | 3 | `GET /api/v9/users/@me/saved-messages`x3 |
| `include_inactive` | 3 | `GET /api/v9/users/@me/billing/subscriptions`x3 |
| `exclude_unpaid_statuses` | 3 | `GET /api/v9/users/@me/billing/subscriptions`x3 |
| `subscription_type` | 3 | `GET /api/v9/users/@me/billing/subscriptions`x3 |

Illustrative examples (real strings from snapshot; full URLs in `rest-d02.json` -> `endpoints[].requestIds`):

- `GET /applications/public?application_ids=…` x7 — batch query; up to 15 repeated `application_ids` per request (58 parameter occurrences total).
- `GET /users/{id}/profile?type=sidebar&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false` — sidebar variant; `type=account_popout` with `false` — popout variant. The `type` query parameter switches response layout.
- `GET /channels/{id}/messages` — pagination: `?limit=12` x8, `?before=<snowflake>&limit=24` x2, `?limit=30&around=<snowflake>` x1. That is, `limit` plus one of cursor parameters `before`/`after`/`around`.
- `GET /quests/decision?placement=1&client_heartbeat_session_id=<uuid>&client_ad_session_id=<uuid>&visible_guild_ids=<snowflake>xN` — 7 values of `visible_guild_ids` in one call.
- `GET /games/autocomplete?q=<URL-encoded>&filter_group=default` — 6 calls; query strings entered by user with debounced keystroke replays.

## 4. Request Payloads (postData)

Payloads captured across **6** routes (13 requests total, all with `Content-Type: application/json`). Below are top-level JSON keys with sanitized examples <= 200 characters and requestId citations.

Total `postData` entries in snapshot: 120 (88 `/science`, 13 other API routes, remainder Cloudflare jsd). Lengths: min 2, median 834, max 4001 — the capture harness truncates bodies at 4000 characters: 12 requests hit the ceiling.

Truncated payloads (crucial for schemas):

| requestId | Route | Length |
|---|---|---|
| `run2|95974.2116` | `POST /cdn-cgi/challenge-platform/h/b/jsd/oneshot/d2d3316b729e/0.6136024352790642:1792386030:xXSD0q7OtCBFmyu7mgupgOBkHgWGMGnre-kB2YvXedE/a4cdb9214a6c3675` | 4001 |
| `run2|95974.2417` | `POST /api/v9/science` | 4001 |
| `run2|95974.3413` | `POST /api/v9/science` | 4001 |
| `run2|95974.4044` | `POST /api/v9/science` | 4001 |
| `run2|95974.7478` | `POST /api/v9/science` | 4001 |
| `run2|95974.7951` | `POST /api/v9/science` | 4001 |
| `run2|95974.7990` | `POST /api/v9/science` | 4001 |
| `run2|95974.8047` | `POST /api/v9/science` | 4001 |
| `run2|95974.8163` | `POST /api/v9/science` | 4001 |
| `run2|95974.8275` | `PATCH /api/v9/users/@me/settings-proto/2` | 4001 |
| `ready|57992.7285` | `POST /api/v9/science` | 4001 |
| `ready|57992.7574` | `POST /api/v9/science` | 4001 |

### POST `/channels/{id}/messages/{id}/ack` x5

- Top-level keys: `token`, `last_viewed`
- Example (requestId `ready|57992.7577`):

```json
{"token":null,"last_viewed":4310}
```

### POST `/users/@me/billing/user-offer` x3

- Top-level keys: `allow_triggers`
- Example (requestId `ready|57992.7308`):

```json
{"allow_triggers":true}
```

### PUT `/guilds/{id}/members/@me` x2

- Top-level keys: `(empty-object)`
- Example (requestId `ready|57992.6989`):

```json
{}
```

### POST `/beaker` x1

- Top-level keys: `token`, `events`
- Example (requestId `run2|95974.8268`):

```json
{"token":"REDACTED","events":[{"type":"client_telemetry","properties":{"client_track_timestamp":1792392393947,"rpc_success_count":26,"rpc_failure_count":0,"first_seen_event_sequence_number":0,"last_se…
```

### PATCH `/channels/{id}/explicit-media` x1

- Top-level keys: `message_ids`
- Example (requestId `run2|95974.8279`):

```json
{"message_ids":["{id}"]}
```

### PATCH `/users/@me/settings-proto/{num}` x1

- Top-level keys: `(non-json)`
- Example (requestId `run2|95974.8275`):

```json
{"settings":"REDACTED…
```

Payload observations:

- `POST /science` and `POST /beaker` share the same telemetry schema: `{token, events:[{type, properties:{...}}]}`. Example event types: `libdiscore_loaded`, `app_web_perf_startup_metrics`, `client_telemetry` (requestId `run2|95974.2417`, `run2|95974.8268`).
- `POST /channels/{id}/messages/{id}/ack` — `{token, last_viewed}`; `token` can be `null`, `last_viewed` is message index/offset (requestId `ready|57992.7577`).
- `PUT /guilds/{id}/members/@me?lurker=true` — body is an **empty JSON object** `{}` (requestId `ready|57992.6989`, `run2|95974.7409`). Crucial edge case: body present, zero fields.
- `PATCH /channels/{id}/explicit-media` — `{message_ids:[snowflake]}` (requestId `run2|95974.8279`).
- `PATCH /users/@me/settings-proto/{num}` — body is `{"settings":"<base64 protobuf>"}`, truncated in snapshot at 4001 characters, marked `(non-json)`. Inner structure is encoded protobuf (requestId `run2|95974.8275`).

## 5. Response Bodies

**Critical limitation:** The snapshot contains 200 `body` events, and all 200 are `/assets/*.js` static chunks (webpack script chunks on host `discord.com`). Zero REST endpoint response bodies were captured.

Independent proof:

```bash
jq -r 'select(.ev=="body") | .url' d02-snapshot/capture-ready.snap.jsonl | wc -l        # 200
jq -r 'select(.ev=="body") | .url' d02-snapshot/capture-ready.snap.jsonl | grep -vc '/assets/'                                                              # 0
jq -r 'select(.ev=="body") | (.body|tostring|length)' d02-snapshot/capture-ready.snap.jsonl | sort -n | awk 'NR==1{m=$1} {a[NR]=$1} END{print NR, m, $1, a[int(NR/2)]}'            # 200 141 20001 520
```

Body lengths: min 141, median 520, max **20,001** (script ceiling: 20,000 characters + newline), total 367,961 characters. 200 of 200 responses returned HTTP 200.

Consequences for D-02:

- Response schemas for `READY`, `/gateway`, `/users/@me`, `/channels/*/messages`, `/science`, `/experiments` cannot be extracted from this capture.
- WebSocket frames in snapshot: 494 `wsrecv` (471 binary + 23 text), 19 `wssent`, 120 `wssent-bin`, 6 `wsc`. Binary frames are base64 of zstd-compressed ETF (`firstHex` starts with `28b52ffd` — zstd magic), meaning **READY body in snapshot is binary-encoded**; text frames belong to voice socket (see section 6.5).
- `POST /science` reconstructs telemetry request-side schema (section 4) — the only rich schema in this snapshot.
- `GET /users/@me` (x3) has no captured body.

## 6. Authentication Flow

### 6.1 Empirical Observations

| Indicator | Value |
|---|---|
| Requests with `Authorization` header | 221 |
| — of which key is `Authorization` (uppercase) | 218 |
| — of which key is `authorization` (lowercase) | 3 — all 3 on `GET /api/v9/users/@me` |
| With `X-Super-Properties` | 221 |
| With `X-Discord-Locale` | 221 |
| With `X-Discord-Timezone` | 221 |
| With `X-Debug-Options` | 221 |
| With `Cookie` | 0 (see U-05) |
| `POST /api/v9/science` with Authorization | 88 of 88 — telemetry is authenticated |
| Requests to `/api/v9/auth/*` | 0 (see U-02) |
| Requests to `GET /api/v9/gateway` | 0 (see U-03) |

Key finding: **All 133 API requests without exception** carry the identical header set — `Authorization` + `X-Super-Properties` + `X-Discord-Locale` + `X-Discord-Timezone` + `X-Debug-Options` + `Accept-Language` (`requestsWithFullAuthHeaderSet = 133`). This matches the header envelope constructed by `prepareRequest` (rest-static section 4, module 926456). Exactly 13 requests carry `Content-Type: application/json` — precisely those with request bodies.

Full list of authenticated endpoints in `rest-d02.json` -> `auth.endpointsWithAuth` (47 routes).

### 6.2 Super Properties (base64 JSON) — Decoded Example

requestId `run2|95974.2323` (`GET /api/v9/channels/{id}/messages?limit=12`), present on 221 requests (all authenticated — 133 API + 88 science):

```json
{
  "os": "Linux",
  "browser": "Discord Client",
  "release_channel": "stable",
  "client_version": "1.0.162",
  "os_version": "7.2.10-1-cachyos",
  "os_arch": "x64",
  "app_arch": "x64",
  "system_locale": "en",
  "has_client_mods": false,
  "client_launch_id": "ab3d725a-82e4-4d66-b77c-64a8463fb04c",
  "browser_user_agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.162 Chrome/148.0.7778.280 Electron/42.11.12 Safari/537.36",
  "browser_version": "42.11.12",
  "window_manager": "KDE,unknown",
  "distro": ""Arch Linux"",
  "runtime_environment": "native",
  "display_server": "wayland",
  "client_build_number": 641004,
  "native_build_number": 94186,
  "client_event_source": null,
  "launch_signature": "da02ad21-144b-43b5-9618-a18392f1c066",
  "client_heartbeat_session_id": "f615427c-47a2-49ed-9482-d4b192dd781e",
  "client_app_state": "focused"
}
```

Field analysis (22 keys):

| Field | Example Value | Meaning |
|---|---|---|
| `os` | `Linux` | Client OS |
| `browser` | `Discord Client` | Client application name |
| `release_channel` | `stable` | Release channel |
| `client_version` | `1.0.162` | Discord version |
| `os_version` | `7.2.10-1-cachyos` | Kernel version |
| `os_arch` | `x64` | OS architecture |
| `app_arch` | `x64` | Binary architecture |
| `system_locale` | `en` | System locale |
| `has_client_mods` | `false` | Client mod detection flag |
| `client_launch_id` | `ab3d725a-82e4-4d66-b77c-64a8463fb04c` | Launch session UUID |
| `browser_user_agent` | `Mozilla/5.0 ...` | Complete User-Agent |
| `browser_version` | `42.11.12` | Chromium/Electron version |
| `window_manager` | `KDE,unknown` | Window manager identification |
| `distro` | `"Arch Linux"` | Distribution name |
| `runtime_environment` | `native` | native (Electron) vs web |
| `display_server` | `wayland` | wayland/x11 |
| `client_build_number` | `641004` | Web client build number |
| `native_build_number` | `94186` | Native module build number |
| `client_event_source` | `null` | Event source trigger |
| `launch_signature` | `da02ad21-144b-43b5-9618-a18392f1c066` | Launch signature UUID |
| `client_heartbeat_session_id` | `f615427c-47a2-49ed-9482-d4b192dd781e` | Telemetry session UUID |
| `client_app_state` | `focused` | Window focus state |

Headers referenced in static code but absent from live traffic: `X-Fingerprint`/`X-Installation-ID`/`X-Routing-Key`/`x-client-trace-id` (see U-08). Observed headers included `X-Context-Properties` (base64, 8 requests across 3 routes: `quests/decision`x3, `quests/get-decisions`x3, `guilds/{id}/members/@me`x2) and `x-science-test`/`X-Failed-Requests` (solely on `/science`).

### 6.3 API Version v9 Resolution

- Empirical: **All 133 API requests** target `https://discord.com/api/v9/...` (zero v6/v8/v10 requests). Verified via `jq -r 'select(.ev=="req")|.url' … | grep -o '/api/v[0-9]*' | sort | uniq -c`.
- Static: `"https:" + window.GLOBAL_ENV.API_ENDPOINT + "/v" + window.GLOBAL_ENV.API_VERSION` (rest-static section 2, module 636537).
- The capture confirms `API_VERSION = 9`, while the origin constant (`GLOBAL_ENV` in `web.js`) resides outside captured response bodies.

### 6.4 Absent Auth Operations

- No `POST /auth/login`, `/auth/mfa/*`, `/auth/register`, `/auth/logout`, `GET /auth/sessions` — active session was pre-existing; token loaded from `TokenStore` (localStorage, rest-static section 4).
- No `POST /auth/handoff/exchange` or `GET /oauth2/*`.
- No `GET /api/v9/gateway` — gateway URL provided via `GLOBAL_ENV.GATEWAY_ENDPOINT` (see U-03); only raw socket visible: `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream`.

### 6.5 Secondary Auth Channel: Voice Gateway (Empirical Proof)

While main gateway frames are binary (zstd+ETF), the voice gateway socket is **plain text JSON**, revealing explicit credentials in transit:

- Socket: `wss://c-fra19-4da7fe5f.discord.media:2053/?v=9` (requestId `run2|95974.8158`), second: `ready|57992.7568`.
- `op:0` (Identify, client to server) — payload:

```json
{"op":0,"d":{"server_id":"{guild_id}","channel_id":"{channel_id}","user_id":"{user_id}","session_id":"REDACTED","token":"REDACTED","max_dave_protocol_version":1,"video":true,"streams":[{"type":"video","rid":"100","quality":100}]}}
```

- `op:1` (Select Protocol): `{protocol:"udp", data:{address, port, mode:"aead_aes256_gcm_rtpsize"}, codecs:[{name:"opus",type:"audio",payload_type:120},{name:"AV1",type:"video"}]}` — client provides encryption mode and codecs.
- `op:2` (Ready, server to client): `{streams:[{ssrc,rtx_ssrc,rid:"100"}], ssrc, port, modes:["aead_aes256_gcm_rtpsize","aead_xchacha20_poly1305_rtpsize"]}`.
- `op:4` (Session Description): `{video_codec:"AV1", secure_frames_version:1, secret_key:"REDACTED", mode:"aead_aes256_gcm_rtpsize", media_session_id:"REDACTED", dave_protocol_version:1, audio_codec:"opus"}`.
- `op:8` (Hello): `{v:8, heartbeat_interval:13750}` — **voice gateway protocol version is v8 while REST is v9**; heartbeat interval is 13,750 ms vs 41,250 ms on main gateway.
- `op:16` (Versions): `{voice:"0.22.1+denbeigh.remove-voice-curator-74a3bd0c5", rtc_worker:"1.6.98"}`.
- `op:3` (heartbeat, seq_ack) and `op:6` (ack) — 7 times each; `op:15` `{any:100}` (media sink wants).

Conclusion: No `/voice/*` REST calls exist (U-09), but the **secondary voice auth path** is empirically confirmed: `token` + `session_id` are delivered over voice gateway `op:0`.

## 7. Filtered Non-API Noise

Filtered **6521** requests (98.0 % of all `req`). Excluded from primary API tables.

| Group | Requests | Domains | Types |
|---|---|---|---|
| `static-assets` | 6025 | discord.comx6025 | Scriptx4264, Fetchx5, Stylesheetx1619, Fontx45, Imagex78, Mediax11, XHRx3 |
| `cdn-media` | 334 | cdn.discordapp.comx308, media.discordapp.netx4, images-ext-1.discordapp.netx22 | Imagex314, Otherx6, XHRx9, Fetchx5 |
| `telemetry-science` | 88 | discord.comx88 | XHRx88 |
| `data-url` | 40 | (inline-data)x40 | Imagex40 |
| `third-party` | 25 | latency.discord.mediax3, scopex18, i.scdn.cox2, startx2 | XHRx3, Fetchx20, Imagex2 |
| `cloudflare-challenge` | 4 | discord.comx4 | Scriptx2, XHRx1, Otherx1 |
| `discord-other-subdomain` | 3 | status.discord.comx3 | XHRx3 |
| `web-navigation` | 2 | discord.comx2 | Documentx2 |

Breakdown by subgroup:

- **`static-assets` (6025)** — Webpack bundles: `Script` 4264, `Stylesheet` 1619, `Image` 78, `Font` 45, `Media` 11, `XHR` 3, `Fetch` 5. Warmup/reloads of `web.js` + lazy chunks (`/assets/<num>.<hash>.js`, `*.module.wasm`).
- **`cdn-media` (334)** — `cdn.discordapp.com` 308 (`/media/v1/collectibles-shop/{id}/static` x44, `/avatars/{id}/<hash>.webp`, `/icons/{id}/…`, `/clan-badges/{id}/…`, `/detectables/games-v1.json` x6, `non-games-v1.json` x6), `images-ext-1.discordapp.net` 22, `media.discordapp.net` 4.
- **`telemetry-science` (88)** — `POST /api/v9/science`, all XHR, all authenticated. Destination for analytics (`libdiscore_loaded`, `app_web_perf_startup_metrics`, `fetch_user_offer_started`, `client_telemetry`). `/api/v9/beaker` — 1 request with same schema.
- **`data-url` (40)** — `data:image/png;base64,…` (inline PNG, non-network).
- **`third-party` (25)** — `sentry-ipc://scope/sentry_key` x18 and `sentry-ipc://start/sentry_key` x2 (IPC channel Electron -> main for Sentry), `latency.discord.media/rtc?v=2` x3 (RTC latency probing), `i.scdn.co` x2 (Spotify media).
- **`cloudflare-challenge` (4)** — `/cdn-cgi/challenge-platform/...` (Cloudflare jsd anti-bot scripts).
- **`discord-other-subdomain` (3)** — `status.discord.com/api/v2/scheduled-maintenances/upcoming.json` (v2 status API, intentionally separated from v9).
- **`web-navigation` (2)** — Transitions `GET /channels/{guild}` (Document), non-XHR.

Noise volume ranking:

| Path | Requests |
|---|---|
| `/api/v9/science` | 88 |
| `/media/v1/collectibles-shop/{id}/static` | 44 |
| `(data: URL, inline PNG — non-network)` | 40 |
| `/sentry_key` | 20 |
| `/avatars/{id}/e234430751874dcfca60cdd3a53a2673.webp` | 14 |
| `/avatars/{id}/a_49f72f47b64c72f2f130cb9fb2808cc1.webp` | 6 |
| `/detectables/games-v1.json` | 6 |
| `/detectables/non-games-v1.json` | 6 |
| `/media/v1/collectibles-shop/{id}/animated` | 6 |
| `/assets/web.4d4b2f75fdea5470.js` | 5 |
| `/icons/{id}/81805511d8bc5fb4680727e99f041773.webp` | 5 |
| `/icons/{id}/b4b4c3850182f7e0dbbaa116df2ed3f2.webp` | 5 |

## 8. Static Reconciliation and Unknowns

### 8.1 Reconciliation

- Empirical: **47** unique routes / **133** invocations.
- Static: 648 REST records, of which 626 have defined methods; 10 records contain extraction artifacts (`${null!=e?` etc.).

| Reconciliation Result | Routes |
|---|---|
| Confirmed by static map (path + method) | **44** |
| Path present, method undefined (`null`) in static map — empirical data clarifies | 2 |
| Method conflict (static vs live discrepancy) | 0 |
| Absent from static map (new empirical discovery) | 1 |

#### Methods Undefined in Static Map, Resolved Empirically

| Empirical Method | Path | Calls | Static Key |
|---|---|---|---|
| **GET** | `/guilds/{id}/application-command-index` | 2 | APPLICATION_COMMAND_INDEX_GUILD |
| **PUT** | `/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me` | 1 | REACTION |

#### Routes Absent from Static Map

| Method | Path | Calls | Notes |
|---|---|---|---|
| POST | `/beaker` | 1 | Secondary telemetry channel (same `{token, events}` schema as `/science`), missing from static map |

#### Routes Located Post Path Normalization

| Empirical Method | Empirical Path | Static Path (Raw) | requestId |
|---|---|---|---|
| GET | `/content-inventory/users/@me` | `/content-inventory/users/@me${null!=e?` | run2|95974.3434 |

### 8.2 Expected Major Routes Unobserved in Capture

| Route | Why Expected | Status in Capture |
|---|---|---|
| `/auth/login` | Primary login (POST) | Unconfirmed |
| `/auth/register` | Registration (POST) | Unconfirmed |
| `/auth/logout` | Session termination (POST) | Unconfirmed |
| `/auth/mfa/*` | Multi-factor auth (POST) | Unconfirmed |
| `/auth/sessions` | Session listing (GET) | Unconfirmed |
| `/auth/location-metadata` | Region/captcha preflight (GET) | Unconfirmed |
| `/experiments` | Feature flags; when authenticated, delivered via READY | Unconfirmed |
| `/apex/experiments` | Web app surface flags (GET) | Unconfirmed |
| `/gateway` | REST gateway URL lookup (GET) | Unconfirmed |
| `/users/@me/settings` | User settings (GET/PATCH) | Unconfirmed |
| `/oauth2/authorize` | OAuth2 consent prompt | Unconfirmed |
| `/voice/regions` | Voice region listing | Unconfirmed |
| `/read-states/ack-bulk` | Bulk read state acknowledgment | Unconfirmed |
| `/track` | Telemetry tracking (POST) | Unconfirmed |

Note: `POST /api/v9/science` occurred **88 times** (all authenticated), but classified as telemetry rather than REST API (section 7). `POST /api/v9/track` did not occur.

### 8.3 Unknowns Ledger

| # | Topic | Detail | Status |
|---|---|---|---|
| U-01 | REST Response Bodies | Zero `body` events for `/api/*` in either capture. All 200 bodies were `/assets/*.js`. | Unconfirmed |
| U-02 | `/api/v9/auth/*` | Zero requests to `/auth/*`: active session pre-existed. | Unconfirmed |
| U-03 | `GET /api/v9/gateway` | Absent. Client directly accesses gateway socket. | Unconfirmed |
| U-04 | User Agent Divergence | Minor discrepancies between platform reports. | Resolved |
| U-05 | Cookie headers | Zero `req` events contained `Cookie` headers (CDP restriction). | Unconfirmed |
| U-06 | Authorization Token Prefix | Token values sanitized to REDACTED in all captures. | Static-only |
| U-07 | Authorization case variance | 3 requests used lowercase `authorization`. | Unconfirmed |
| U-08 | Telemetry headers | Static headers `X-Fingerprint` absent in empirical API calls. | Divergence |
| U-09 | Voice REST API | Voice operates over text WS (v=8) rather than REST endpoints. | Verified |
| U-10 | Capture appending | `capture-run2.jsonl` was appended live; analysis bound to snapshot. | Verified |

## 9. Appendix: Reproducibility Guide

```bash
# 1. Freeze snapshot
mkdir -p docs/re/d02-snapshot && cd docs/re
cp capture-run2.jsonl d02-snapshot/capture-run2.snap.jsonl
cp capture-ready.jsonl d02-snapshot/capture-ready.snap.jsonl
md5sum d02-snapshot/*.jsonl

# 2. Parse and generate report
node rest-d02.mjs && node rest-d02-report.mjs

# 3. Independent numerical verification (jq/awk/join)
bash rest-d02-verify.sh | tee rest-d02.verify.txt
```

Expected verification results (from `rest-d02.verify.txt`):

```
req=6654 (4395+2259), unique requestId=6651 (3 retries due to redirect)
API v9: 133 requests, 47 unique routes; /api/v2: 3 (status.discord.com)
Authorization: 218 Authorization + 3 authorization = 221
X-Super-Properties / X-Discord-Timezone / X-Discord-Locale / X-Debug-Options / Accept-Language = 221 each
science: 88 requests, all authenticated, all HTTP 204
body: 200, non-asset 0, min 141 / median 520 / max 20001 / sum 367961 (9 hit 20000 limit)
postData: 120, truncation at 4000 (12 hit limit); API (non-science) with body: 13
noise: assets=6025 cloudflare=4 data=40 web-nav=2 cdn+thirdparty=362
API status codes: 200x113, 204x8, 304x3, 404x8, 429x1
```

Artifacts catalog:

| File | Content |
|---|---|
| `rest-d02.md` | This empirical report |
| `rest-d02.json` | Machine-readable outputs: `endpoints[]`, `queryParams`, `auth`, `noise`, `crossRef`, `unknowns` |
| `rest-d02.stats.json` | Flat counters and endpoint lists |
| `rest-d02.crossref.json` | Detailed route cross-referencing against static map |
| `rest-d02.verify.txt` | Output of independent verification script (`rest-d02-verify.sh`) |
| `rest-d02.mjs`, `rest-d02-report.mjs`, `rest-d02-verify.sh` | Parser, report generator, and verification shell scripts |
| `d02-snapshot/` | Frozen input data snapshot and checksums |
